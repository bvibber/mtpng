//
// mtpng - a multithreaded parallel PNG encoder in Rust
// encoder.rs - implements the public encoder interface & internals
//
// Copyright (c) 2018 Brion Vibber
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.
//

use rayon::ThreadPool;

use std::cmp;

use std::collections::HashMap;

use std::io;
use std::io::Write;

use std::sync::Arc;
use std::sync::mpsc;
use std::sync::mpsc::{Sender, Receiver};

use super::ColorType;
use super::CompressionLevel;
use super::Header;
use super::Mode;
use super::Mode::{Adaptive, Fixed};

use super::filter::AdaptiveFilter;
use super::filter::Filter;
use super::writer::Writer;

use super::deflate;
use super::deflate::Deflate;
use super::deflate::Flush;
use super::deflate::Strategy;

use super::utils::*;


#[derive(Copy, Clone)]
pub struct Options<'a> {
    chunk_size: usize,
    compression_level: CompressionLevel,
    strategy_mode: Mode<Strategy>,
    filter_mode: Mode<Filter>,
    streaming: bool,
    thread_pool: Option<&'a ThreadPool>,
}

impl<'a> Options<'a> {
    // Use default options
    pub fn new() -> Options<'a> {
        Options {
            //
            // A chunk size of 256 KiB gives compression results very similar
            // to a single stream when otherwise using defaults.
            //
            chunk_size: 256 * 1024,

            //
            // Same defaults as libpng.
            //
            compression_level: CompressionLevel::Default,
            strategy_mode: Adaptive,
            filter_mode: Adaptive,

            //
            // Streaming mode can produce lower latency to first bytes hitting
            // output on large files, at the cost of size -- several extra
            // 32-bit words per chunk, which adds up.
            //
            // Leaving off will buffer compressed image data into memory until
            // the end is reached.
            //
            streaming: false,

            //
            // Use the global thread pool.
            //
            thread_pool: None,
        }
    }
}

impl<'a> Options<'a> {
    pub fn set_thread_pool(&mut self, thread_pool: &'a ThreadPool) -> IoResult {
        self.thread_pool = Some(thread_pool);
        Ok(())
    }

    pub fn set_chunk_size(&mut self, chunk_size: usize) -> IoResult {
        if chunk_size < 32768 {
            Err(invalid_input("chunk size must be at least 32768"))
        } else {
            self.chunk_size = chunk_size;
            Ok(())
        }
    }

    pub fn set_compression_level(&mut self, level: CompressionLevel) -> IoResult {
        self.compression_level = level;
        Ok(())
    }

    pub fn set_filter_mode(&mut self, filter_mode: Mode<Filter>) -> IoResult {
        self.filter_mode = filter_mode;
        Ok(())
    }

    pub fn set_strategy_mode(&mut self, strategy_mode: Mode<Strategy>) -> IoResult {
        self.strategy_mode = strategy_mode;
        Ok(())
    }

    pub fn set_streaming(&mut self, streaming: bool) -> IoResult {
        self.streaming = streaming;
        Ok(())
    }
}

// Accumulates a set of pixels, then gets sent off as input
// to the deflate jobs.
struct PixelChunk {
    header: Header,

    index: usize,
    start_row: usize,
    end_row: usize,
    is_start: bool,
    is_end: bool,

    stride: usize,

    // Rows of pixel data, each with stride bytes per row
    rows: Vec<Vec<u8>>,
}

impl PixelChunk {
    fn new(header: Header, index: usize, start_row: usize, end_row: usize) -> PixelChunk {
        assert!(start_row <= end_row);

        let height = header.height as usize;
        assert!(end_row <= height);

        PixelChunk {
            header: header,

            index: index,
            start_row: start_row,
            end_row: end_row,
            is_start: start_row == 0,
            is_end: end_row == height,

            stride: header.stride(),

            rows: Vec::with_capacity(end_row - start_row),
        }
    }

    fn is_full(&self) -> bool {
        self.rows.len() == (self.end_row - self.start_row)
    }

    fn read_row(&mut self, row: &[u8])
    {
        let mut row_copy = Vec::with_capacity(self.stride);
        row_copy.extend_from_slice(row);

        self.rows.push(row_copy);
    }

    fn get_row(&self, row: usize) -> &[u8] {
        if row < self.start_row {
            panic!("Tried to access row from earlier chunk: {} < {}", row, self.start_row);
        } else if row >= self.end_row {
            panic!("Tried to access row from later chunk: {} >= {}", row, self.end_row);
        } else {
            return &self.rows[row - self.start_row];
        }
    }
}

// Takes pixel chunks as input and accumulates filtered output.
struct FilterChunk {
    index: usize,
    start_row: usize,
    end_row: usize,
    is_start: bool,
    is_end: bool,

    stride: usize,
    filter_mode: Mode<Filter>,

    // The input pixels for chunk n-1
    // Needed for its last row only.
    prior_input: Option<Arc<PixelChunk>>,

    // The input pixels for chunk n
    input: Arc<PixelChunk>,

    // Filtered output bytes
    data: Vec<u8>,
}

impl FilterChunk {
    fn new(prior_input: Option<Arc<PixelChunk>>,
           input: Arc<PixelChunk>,
           filter_mode: Mode<Filter>) -> FilterChunk
    {
        // Prepend one byte for the filter selector.
        let stride = input.stride + 1;
        let nbytes = stride * (input.end_row - input.start_row);

        FilterChunk {
            index: input.index,
            start_row: input.start_row,
            end_row: input.end_row,
            is_start: input.is_start,
            is_end: input.is_end,

            stride: stride,
            filter_mode: filter_mode,

            prior_input: prior_input,
            input: input,
            data: Vec::with_capacity(nbytes),
        }
    }

    // Return the last up-to-32kib, used as an input dictionary
    // for the next chunk's deflate job.
    fn get_trailer(&self) -> &[u8] {
        let trailer = 32768;
        let len = self.data.len();
        if len > trailer {
            return &self.data[len - trailer .. len];
        } else {
            return &self.data[0 .. len];
        }
    }

    //
    // Run the filtering, on a background thread.
    //
    fn run(&mut self) -> IoResult {
        let mut filter = AdaptiveFilter::new(self.input.header, self.filter_mode);
        let zero = vec![0u8; self.stride - 1];
        for i in self.start_row .. self.end_row {
            let prior = if i == self.start_row {
                match self.prior_input {
                    Some(ref input) => &input,
                    None => &self.input, // Won't get used.
                }
            } else {
                &self.input
            };
            let prev = if i == 0 {
                &zero
            } else {
                prior.get_row(i - 1)
            };

            let row = self.input.get_row(i);

            let output = filter.filter(prev, row);

            self.data.write_all(output)?
        }
        Ok(())
    }
}

// Takes filter chunks as input and accumulates compressed output.
struct DeflateChunk {
    index: usize,
    is_start: bool,
    is_end: bool,

    compression_level: CompressionLevel,
    strategy: Strategy,

    // The filtered pixels for chunk n-1
    // Empty on first chunk.
    // Needed for its last row only.
    prior_input: Option<Arc<FilterChunk>>,

    // The filtered pixels for chunk n
    input: Arc<FilterChunk>,

    // Compressed output bytes
    data: Vec<u8>,

    // Checksum of this chunk
    adler32: u32,
}

impl DeflateChunk {
    fn new(compression_level: CompressionLevel,
           strategy: Strategy,
           prior_input: Option<Arc<FilterChunk>>,
           input: Arc<FilterChunk>) -> DeflateChunk {

        DeflateChunk {
            index: input.index,
            is_start: input.is_start,
            is_end: input.is_end,

            compression_level: compression_level,
            strategy: strategy,

            prior_input: prior_input,
            input: input,
            data: Vec::new(),
            adler32: deflate::adler32_initial(),
        }
    }

    fn run(&mut self) -> IoResult {
        // Run the deflate!
        // Todo: don't create an empty vector earlier, but reuse it sanely.
        let data = Vec::<u8>::new();

        let mut options = deflate::Options::new();

        options.set_window_bits(if self.is_start {
            // 15 means 2^15 (32 KiB), the max supported.
            15
        } else {
            // Negative forces raw stream output so we don't get
            // a second header...
            -15
        });

        match self.compression_level {
            CompressionLevel::Default => {},
            CompressionLevel::Fast => options.set_level(1),
            CompressionLevel::High => options.set_level(9),
        }
        options.set_strategy(self.strategy);

        let mut encoder = Deflate::new(options, data);


        match self.prior_input {
            Some(ref filter) => {
                let trailer = filter.get_trailer();
                encoder.set_dictionary(trailer)?;
            },
            None => {
                // do nothing.
            }
        }

        encoder.write(&self.input.data, if self.is_end {
            Flush::Finish
        } else {
            Flush::SyncFlush
        })?;

        // In raw deflate mode we have to calculate the checksum ourselves.
        self.adler32 = deflate::adler32(1, &self.input.data);

        return match encoder.finish() {
            Ok(data) => {
                // This seems lame to move the vector back, but it's actually cheap.
                self.data = data;
                Ok(())
            },
            Err(e) => Err(e)
        }
    }
}

//
// List of completed chunks, which may come in in any order
// but are returned in original order, in pairs with the
// prior chunk when available.
//
// The prior chunk is passed around because filtering and
// deflating jobs need the end (last row, or last 32 KiB)
// of the previous chunk's input as well as their own.
//
struct ChunkMap<T> {
    cursor_in: usize,
    cursor_out: usize,

    // todo use a VecDeque for this maybe
    live_chunks: HashMap<usize, Arc<T>>,
}

impl<T> ChunkMap<T> {
    fn new() -> ChunkMap<T> {
        ChunkMap {
            cursor_in: 0,
            cursor_out: 0,
            live_chunks: HashMap::new(),
        }
    }

    fn in_flight(&self) -> bool {
        self.cursor_in > self.cursor_out
    }

    //
    // Record that this job is now in-flight
    //
    fn advance(&mut self) {
        self.cursor_in = self.cursor_in + 1;
    }

    //
    // Record that this job has landed and save its data.
    //
    fn land(&mut self, index: usize, chunk: Arc<T>) {
        if index < self.cursor_out {
            panic!("Tried to land an expired chunk");
        }
        if index > self.cursor_in {
            panic!("Tried to land a future chunk");
        }
        match self.live_chunks.insert(index, chunk) {
            None => {},
            Some(_x) => panic!("Tried to re-append an existing chunk"),
        }
    }

    fn get(&self, index: usize) -> Option<Arc<T>> {
        match self.live_chunks.get(&index) {
            Some(item) => Some(Arc::clone(item)),
            _ => None,
        }
    }

    fn retire(&mut self, index: usize) {
        self.live_chunks.remove(&index);
    }

    fn pop_front(&mut self) -> Option<(Option<Arc<T>>, Arc<T>)> {
        let index = self.cursor_out;
        let current = self.get(index);
        if index > 0 {
            let prev = self.get(index - 1);
            match (prev, current) {
                (Some(p), Some(c)) => {
                    // Next pipeline stage needs the current
                    // and previous items from this stage.
                    self.cursor_out = self.cursor_out + 1;
                    let prev_chunk = p.clone();
                    let cur_chunk = c.clone();

                    // Drop the previous item off the list;
                    // it'll be kept alive by whatever needs
                    // it while they run.
                    self.retire(index - 1);

                    return Some((Some(prev_chunk), cur_chunk));
                },
                _ => {
                    return None;
                }
            }
        } else {
            match current {
                Some(c) => {
                    self.cursor_out = self.cursor_out + 1;
                    return Some((None, c.clone()));
                },
                _ => {
                    return None;
                }
            }
        }
    }
}

enum ThreadMessage {
    FilterDone(Arc<FilterChunk>),
    DeflateDone(Arc<DeflateChunk>),
    Error(io::Error),
}

#[derive(Copy, Clone)]
enum DispatchMode {
    Blocking,
    NonBlocking,
}

enum RowStatus {
    Continue,
    Done,
}

pub struct Encoder<'a, W: Write> {
    writer: Writer<W>,
    options: Options<'a>,

    header: Header,

    wrote_header: bool,
    wrote_palette: bool,
    palette_length: usize,
    wrote_transparency: bool,
    started_image: bool,

    rows_per_chunk: usize,
    chunks_total: usize,
    chunks_output: usize,

    // Accumulates input rows until enough are ready to fire off a filter job.
    pixel_accumulator: Arc<PixelChunk>,
    pixel_index: usize,
    current_row: u32,

    // Accumulates completed output from pixel input, filter, and deflate jobs.
    pixel_chunks: ChunkMap<PixelChunk>,
    filter_chunks: ChunkMap<FilterChunk>,
    deflate_chunks: ChunkMap<DeflateChunk>,

    // Accumulates the checksum of all output chunks in turn.
    adler32: u32,

    // Accumulates IDAT output when not using streaming output mode
    idat_buffer: Vec<u8>,

    // For messages from the thread pool.
    tx: Sender<ThreadMessage>,
    rx: Receiver<ThreadMessage>,
}

impl<'a, W: Write> Encoder<'a, W> {
    pub fn new(write: W, options: &Options<'a>) -> Encoder<'a, W> {
        let (tx, rx) = mpsc::channel();
        Encoder {
            writer: Writer::new(write),

            header: Header::new(),
            options: options.clone(),

            wrote_header: false,
            wrote_palette: false,
            palette_length: 0,
            wrote_transparency: false,
            started_image: false,

            rows_per_chunk: 0,
            chunks_total: 0,
            chunks_output: 0,

            // hack, clean this up later
            pixel_accumulator: Arc::new(PixelChunk::new(Header::new(), 0, 0, 0)),
            pixel_index: 0,
            current_row: 0,

            pixel_chunks: ChunkMap::new(),
            filter_chunks: ChunkMap::new(),
            deflate_chunks: ChunkMap::new(),

            adler32: deflate::adler32_initial(),
            idat_buffer: Vec::new(),

            tx: tx,
            rx: rx,
        }
    }

    //
    // Flush output and return the Write sink for further manipulation.
    // Consumes the encoder instance.
    //
    pub fn finish(mut self) -> io::Result<W> {
        self.flush()?;
        return if self.is_finished() {
            self.writer.write_end()?;
            self.writer.finish()
        } else {
            Err(other("Incomplete image input"))
        }
    }

    fn dispatch_func<F>(&self, func: F)
        where F: Fn(&Sender<ThreadMessage>) + Send + 'static
    {
        let tx = self.tx.clone();
        match self.options.thread_pool {
            Some(pool) => {
                pool.spawn(move || {
                    func(&tx);
                });
            },
            None => {
                ::rayon::spawn(move || {
                    func(&tx);
                });
            }
        }
    }

    fn start_row(&self, index: usize) -> usize {
        index * self.rows_per_chunk
    }

    fn end_row(&self, index: usize) -> usize {
        cmp::min(self.start_row(index) + self.rows_per_chunk, self.header.height as usize)
    }

    fn receive(&mut self, blocking: DispatchMode) -> Option<ThreadMessage> {
        return match blocking {
            DispatchMode::Blocking => match self.rx.recv() {
                Ok(msg) => Some(msg),
                _ => None,
            },
            DispatchMode::NonBlocking => match self.rx.try_recv() {
                Ok(msg) => Some(msg),
                _ => None,
            }
        }
    }

    fn filter_mode(&self) -> Mode<Filter> {
        match self.options.filter_mode {
            Fixed(s) => Fixed(s),
            Adaptive => match self.header.color_type {
                ColorType::IndexedColor => Fixed(Filter::None),
                _                       => Adaptive,
            }
        }
    }

    fn compression_strategy(&self) -> Strategy {
        match self.options.strategy_mode {
            Fixed(s) => s,
            Adaptive => match self.filter_mode() {
                Fixed(Filter::None) => Strategy::Default,
                _                   => Strategy::Filtered,
            },
        }
    }

    fn dispatch(&mut self, mode: DispatchMode) -> IoResult {
        // See if anything interesting happened on the threads.
        let mut blocking_mode = mode;
        while self.filter_chunks.in_flight() || self.deflate_chunks.in_flight() {
            match self.receive(blocking_mode) {
                Some(ThreadMessage::FilterDone(filter)) => {
                    self.filter_chunks.land(filter.index, filter);
                }
                Some(ThreadMessage::DeflateDone(deflate)) => {
                    self.deflate_chunks.land(deflate.index, deflate);
                },
                Some(ThreadMessage::Error(e)) => {
                    return Err(e);
                }
                None => {
                    // No more output from the threads.
                    break;
                }
            }
            // After the first one, keep reading any if they're there
            // but don't block further.
            blocking_mode = DispatchMode::NonBlocking;
        }

        // If we have output to run, write it!
        loop {
            match self.deflate_chunks.pop_front() {
                Some((_previous, current)) => {
                    if self.chunks_output >= self.chunks_total {
                        panic!("Got extra output after end of file; should not happen.");
                    }

                    // Combine the checksums!
                    self.adler32 = deflate::adler32_combine(self.adler32,
                                                            current.adler32,
                                                            current.input.data.len());

                    // @fixme if not streaming, append to an in-memory buffer
                    // and output a giant tag later.
                    if self.options.streaming {
                        self.writer.write_chunk(b"IDAT", &current.data)?;

                        if current.is_end {
                            let mut chunk = Vec::<u8>::new();
                            if !current.is_start {
                                write_be32(&mut chunk, self.adler32)?;
                            }
                            self.writer.write_chunk(b"IDAT", &chunk)?;
                        }
                    } else {
                        self.idat_buffer.write(&current.data)?;

                        if current.is_end {
                            if !current.is_start {
                                write_be32(&mut self.idat_buffer, self.adler32)?;
                            }
                            self.writer.write_chunk(b"IDAT", &self.idat_buffer)?;
                        }
                    }

                    self.chunks_output = self.chunks_output + 1;
                },
                None => {
                    break;
                },
            }
        }

        // If we have more deflate work to do, dispatch them!
        // @todo check if the thread pool is full and block if so...
        // That will keep memory from growing on large images during fast input.
        loop {
            match self.filter_chunks.pop_front() {
                Some((previous, current)) => {
                    // Prepare to dispatch the deflate job:
                    let level = self.options.compression_level;
                    let strategy = self.compression_strategy();
                    self.deflate_chunks.advance();
                    self.dispatch_func(move |tx| {
                        let mut deflate = DeflateChunk::new(level, strategy, previous.clone(), current.clone());
                        tx.send(match deflate.run() {
                            Ok(()) => ThreadMessage::DeflateDone(Arc::new(deflate)),
                            Err(e) => ThreadMessage::Error(e),
                        }).unwrap();
                    });
                },
                None => {
                    break;
                }
            }
        }

        // If we have more filter work to do, dispatch them!
        loop {
            match self.pixel_chunks.pop_front() {
                Some((previous, current)) => {
                    // Prepare to dispatch the filter job:
                    self.filter_chunks.advance();
                    let filter_mode = self.filter_mode();
                    self.dispatch_func(move |tx| {
                        let mut filter = FilterChunk::new(previous.clone(),
                                                          current.clone(),
                                                          filter_mode);
                        tx.send(match filter.run() {
                            Ok(()) => ThreadMessage::FilterDone(Arc::new(filter)),
                            Err(e) => ThreadMessage::Error(e),
                        }).unwrap();
                    });
                },
                None => {
                    break;
                }
            }
        }

        Ok(())
    }

    //
    // Write the PNG signature and header chunk.
    // Must be done before anything else is output.
    //
    pub fn write_header(&mut self, header: &Header) -> IoResult {
        if self.wrote_header {
            return Err(invalid_input("Cannot write header a second time."));
        }

        self.header = header.clone();

        let stride = self.header.stride() + 1;
        let height = self.header.height as usize;

        let full_rows = self.options.chunk_size / stride;
        let extra_pixels = self.options.chunk_size % stride;
        self.rows_per_chunk = cmp::min(height, full_rows + if extra_pixels > 0 {
            1
        } else {
            0
        });

        let full_chunks = height / self.rows_per_chunk;
        let extra_lines = height % self.rows_per_chunk;
        self.chunks_total = full_chunks + if extra_lines > 0 {
            1
        } else {
            0
        };

        self.pixel_accumulator = Arc::new(PixelChunk::new(self.header, 0, 0, self.rows_per_chunk));

        self.wrote_header = true;

        self.writer.write_signature()?;
        self.writer.write_header(self.header)
    }

    //
    // Write an indexed-color palette as a PLTE chunk.
    //
    // Note this chunk is allowed on truecolor images, though sPLT is preferred.
    //
    pub fn write_palette(&mut self, palette: &[u8]) -> io::Result<()> {
        if !self.wrote_header {
            return Err(invalid_input("Cannot write palette before header."));
        }
        if self.wrote_palette {
            return Err(invalid_input("Cannot write palette a second time."));
        }
        if self.wrote_transparency {
            return Err(invalid_input("Cannot write palette after transparency."));
        }
        if self.started_image {
            return Err(invalid_input("Cannot write palette after image data."));
        }
        if palette.len() < 3 {
            return Err(invalid_input("Palette must have at least one entry."));
        }
        if palette.len() % 3 != 0 {
            return Err(invalid_input("Palette must have an integral number of entries."));
        }

        self.wrote_palette = true;
        self.palette_length = palette.len() / 3;
        self.writer.write_chunk(b"PLTE", palette)
    }

    //
    // Write a transparency info chunk.
    //
    // For indexed color, contains a single alpha value byte per palette
    // entry, up to but not exceeding the number of palette entries.
    //
    // Note this chunk is allowed on greyscale and truecolor images,
    // and there references a single color in 16-bit notation.
    //
    // https://www.w3.org/TR/PNG/#11tRNS
    //
    pub fn write_transparency(&mut self, data: &[u8]) -> io::Result<()> {
        if !self.wrote_header {
            return Err(invalid_input("Cannot write transparency before header."));
        }
        if self.started_image {
            return Err(invalid_input("Cannot write transparency after image data."));
        }
        match self.header.color_type {
            ColorType::Greyscale => {
                if data.len() != 2 {
                    return Err(invalid_input("Greyscale transparency data must be exactly 2 bytes."));
                }
            },
            ColorType::Truecolor => {
                if data.len() != 6 {
                    return Err(invalid_input("Truecolor transparency data must be exactly 6 bytes."));
                }
            },
            ColorType::IndexedColor => {
                if !self.wrote_palette {
                    return Err(invalid_input("Cannot write transparency before palette."));
                }
                if data.len() < 1 {
                    return Err(invalid_input("Transparency data too short."));
                }
                if data.len() > self.palette_length {
                    return Err(invalid_input("Transparency data cannot contain more entries than palette."));
                }
            },
            _ => {
                return Err(invalid_input("Transparency chunk is invalid for color types with alpha"));
            }

        }
        self.wrote_transparency = true;
        self.writer.write_chunk(b"tRNS", data)
    }

    //
    // Copy a row's pixel data into buffers for async compression.
    // Returns immediately after copying.
    //
    fn process_row(&mut self, row: &[u8]) -> io::Result<RowStatus>
    {
        if self.pixel_index >= self.chunks_total {
            return Err(other("invalid internal state"));
        }
        if !self.wrote_header {
            return Err(invalid_input("Cannot write image data before header."));
        }
        match self.header.color_type {
            ColorType::IndexedColor => {
                if !self.wrote_palette {
                    return Err(invalid_input("Cannot write indexed-color image data before palette."));
                }
            },
            _ => {},
        }
        if !self.started_image {
            self.started_image = true;
        }

        Arc::get_mut(&mut self.pixel_accumulator).unwrap().read_row(row);

        if self.pixel_accumulator.is_full() {
            // Move the item off to the completed stack...
            self.pixel_chunks.land(self.pixel_index, self.pixel_accumulator.clone());

            // Make a nice new buffer to accumulate data into.
            self.pixel_index = self.pixel_index + 1;
            if self.pixel_index < self.chunks_total {
                self.pixel_chunks.advance();
                self.pixel_accumulator = Arc::new(PixelChunk::new(self.header,
                                                                  self.pixel_index,
                                                                  self.start_row(self.pixel_index),
                                                                  self.end_row(self.pixel_index)));
            }

            // Dispatch any available async tasks and output.
            self.dispatch(DispatchMode::NonBlocking)?;
        }

        self.current_row = self.current_row + 1;
        if self.current_row == self.header.height {
            Ok(RowStatus::Done)
        } else {
            Ok(RowStatus::Continue)
        }
    }

    //
    // Encode and compress the given image data and write to output.
    // Input data must be packed in the correct format for the given
    // color type and depth, with no padding at the end of rows.
    //
    // An integral number of rows must be provided at once.
    //
    // If not all of the image rows are provided, multiple calls are
    // required to finish out the data.
    //
    pub fn write_image_rows(&mut self, buf: &[u8]) -> IoResult {
        let stride = self.header.stride();
        if buf.len() % stride != 0 {
            Err(invalid_input("Buffer must be an integral number of rows"))
        } else {
            for row in buf.chunks(stride) {
                self.process_row(&mut &*row)?;
            }
            Ok(())
        }
    }

    //
    // Return completion progress as a fraction of 1.0
    //
    pub fn progress(&self) -> f64 {
        self.chunks_output as f64 / self.chunks_total as f64
    }

    //
    // Return finished-ness state.
    // Is it finished? Yeah or no.
    //
    pub fn is_finished(&self) -> bool {
        self.chunks_output == self.chunks_total
    }

    //
    // Flush all currently in-progress data to output
    // Warning: this may block.
    //
    pub fn flush(&mut self) -> IoResult {
        while self.chunks_output < self.pixel_index {
            // Dispatch any available async tasks and output.
            self.dispatch(DispatchMode::Blocking)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::Header;
    use super::super::ColorType;
    use super::Encoder;
    use super::Options;
    use super::IoResult;

    use std::io;

    fn test_encoder<F>(width: u32, height: u32, func: F)
        where F: Fn(&mut Encoder<Vec<u8>>, &[u8]) -> IoResult
    {
        match {
            || -> io::Result<Vec<u8>> {
                let mut data = Vec::<u8>::with_capacity(width as usize * 3);
                for i in 0 .. width as usize * 3 {
                    data.push((i % 255) as u8);
                }

                let writer = Vec::<u8>::new();
                let options = Options::new();
                let mut encoder = Encoder::new(writer, &options);

                let mut header = Header::new();
                header.set_size(width, height).unwrap();
                header.set_color(ColorType::Truecolor, 8).unwrap();
                encoder.write_header(&header)?;

                func(&mut encoder, &data)?;
                encoder.finish()
            }()
        } {
            Ok(_writer) => {},
            Err(e) => assert!(false, "Error {}", e),
        }
    }

    #[test]
    fn create_and_state() {
        test_encoder(1920, 1080, |encoder, data| {

            assert_eq!(encoder.is_finished(), false);
            assert_eq!(encoder.progress(), 0.0);

            // We must finish out the file or it'll whinge.
            for _y in 0 .. 1080 {
                encoder.write_image_rows(data)?;
            }

            Ok(())
        });
    }

    #[test]
    fn test_rows() {
        test_encoder(1920, 1080, |encoder, data| {
            assert_eq!(encoder.is_finished(), false);
            assert_eq!(encoder.progress(), 0.0);

            for _y in 0 .. 1080 {
                encoder.write_image_rows(data)?;
            }

            // Should trigger all blocks!
            encoder.flush()?;
            assert_eq!(encoder.is_finished(), true);
            assert_eq!(encoder.progress(), 1.0);

            Ok(())
        });
    }
}
