// Experimental parallel PNG writer
// Brion Vibber 2018-09-02

use rayon::ThreadPool;

use std::cmp;

use std::collections::HashMap;

use std::io;
use std::io::Write;
type IoResult = io::Result<()>;

use std::sync::Arc;
use std::sync::mpsc;
use std::sync::mpsc::{Sender, Receiver};

use super::Header;
use super::Options;
use super::CompressionLevel;
use super::filter::AdaptiveFilter;
use super::writer::Writer;


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

    // Pixel data, stride bytes per row
    data: Vec<u8>,
}

impl PixelChunk {
    fn new(header: Header, index: usize, start_row: usize, end_row: usize) -> PixelChunk {
        if start_row > end_row {
            panic!("Invalid start row");
        }

        let height = header.height as usize;
        if end_row > height {
            panic!("Invalid end row");
        }

        let stride = header.stride();
        let nbytes = stride * (end_row - start_row);

        PixelChunk {
            header: header,

            index: index,
            start_row: start_row,
            end_row: end_row,
            is_start: start_row == 0,
            is_end: end_row == height,

            stride: stride as usize,

            data: Vec::with_capacity(nbytes),
        }
    }

    fn is_full(&self) -> bool {
        self.data.len() == self.stride * (self.end_row - self.start_row)
    }

    fn append_row(&mut self, row: &[u8]) {
        // If shifts or byte swapping are necessary, during copy is a good place.
        // Otherwise, just copy!
        if row.len() != self.stride {
            panic!("Appending row of wrong stride.");
        } else if self.is_full() {
            panic!("Appending beyond end of chunk.");
        } else {
            self.data.extend_from_slice(row);
        }
    }

    fn get_row(&self, row: usize) -> &[u8] {
        if row < self.start_row {
            panic!("Tried to access row from earlier chunk: {} < {}", row, self.start_row);
        } else if row >= self.end_row {
            panic!("Tried to access row from later chunk: {} >= {}", row, self.end_row);
        } else {
            let start = self.stride * (row - self.start_row);
            return &self.data[start .. start + self.stride];
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
           input: Arc<PixelChunk>) -> FilterChunk {
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

            prior_input: prior_input,
            input: input,
            data: Vec::with_capacity(nbytes),
        }
    }

    // Return ref to all data
    fn get_data(&self) -> &[u8] {
        return &self.data;
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
    fn run(&mut self) {
        let mut filter = AdaptiveFilter::new(self.input.header);
        let zero = vec![0u8; self.stride];
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

            self.data.write(output).unwrap(); // @fixme handle errors
        }
    }
}

// Takes filter chunks as input and accumulates compressed output.
struct DeflateChunk {
    index: usize,
    start_row: usize,
    end_row: usize,
    is_start: bool,
    is_end: bool,

    compression_level: CompressionLevel,

    // The filtered pixels for chunk n-1
    // Empty on first chunk.
    // Needed for its last row only.
    prior_input: Option<Arc<FilterChunk>>,

    // The filtered pixels for chunk n
    input: Arc<FilterChunk>,

    // Compressed output bytes
    data: Vec<u8>,
}

impl DeflateChunk {
    fn new(compression_level: CompressionLevel,
           prior_input: Option<Arc<FilterChunk>>,
           input: Arc<FilterChunk>) -> DeflateChunk {

        DeflateChunk {
            index: input.index,
            start_row: input.start_row,
            end_row: input.end_row,
            is_start: input.is_start,
            is_end: input.is_end,

            compression_level: compression_level,
            prior_input: prior_input,
            input: input,
            data: Vec::new(),
        }
    }

    fn run(&mut self) {
        // -> run the deflating
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
}

#[derive(Copy, Clone)]
enum DispatchMode {
    Blocking,
    NonBlocking,
}

pub struct State<'a, W: Write> {
    header: Header,
    options: Options,
    writer: Writer<W>,
    thread_pool: Option<&'a ThreadPool>,

    rows_per_chunk: usize,
    chunks_total: usize,
    chunks_output: usize,

    // Accumulates input rows until enough are ready to fire off a filter job.
    pixel_accumulator: Arc<PixelChunk>,
    pixel_index: usize,

    // Accumulates completed output from pixel input, filter, and deflate jobs.
    pixel_chunks: ChunkMap<PixelChunk>,
    filter_chunks: ChunkMap<FilterChunk>,
    deflate_chunks: ChunkMap<DeflateChunk>,

    // For messages from the thread pool.
    tx: Sender<ThreadMessage>,
    rx: Receiver<ThreadMessage>,
}

impl<'a, W: Write> State<'a, W> {
    pub fn new(header: Header, options: Options, write: W, thread_pool: Option<&'a ThreadPool>) -> State<'a, W> {
        let stride = header.stride() + 1;

        let full_rows = options.chunk_size / stride;
        let extra_pixels = options.chunk_size % stride;
        let rows_per_chunk = full_rows + if extra_pixels > 0 {
            1
        } else {
            0
        };

        let full_chunks = header.height as usize / rows_per_chunk;
        let extra_lines = header.height as usize % rows_per_chunk;
        let chunks_total = full_chunks + (if extra_lines > 0 {
            1
        } else {
            0
        });

        let mut writer = Writer::new(write);
        // @fime move these to another func
        writer.write_signature();
        writer.write_header(header);

        let (tx, rx) = mpsc::channel();

        State {
            header: header,
            options: options,
            writer: writer,
            thread_pool: thread_pool,

            rows_per_chunk: rows_per_chunk,
            chunks_total: chunks_total,
            chunks_output: 0,

            pixel_accumulator: Arc::new(PixelChunk::new(header, 0, 0, rows_per_chunk)),
            pixel_index: 0,

            pixel_chunks: ChunkMap::new(),
            filter_chunks: ChunkMap::new(),
            deflate_chunks: ChunkMap::new(),

            tx: tx,
            rx: rx,
        }
    }

    pub fn close(mut this: State<W>) -> io::Result<W> {
        this.flush()?;
        Writer::close(this.writer)
    }

    fn dispatch_func<F>(&self, func: F)
        where F: Fn(&Sender<ThreadMessage>) + Send + 'static
    {
        let tx = self.tx.clone();
        match self.thread_pool {
            Some(pool) => {
                pool.install(move || {
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
                    // @fixme if not streaming, append to an in-memory buffer
                    // and output a giant tag later.
                    self.writer.write_chunk(b"IDAT", &current.data)?;
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
                    self.deflate_chunks.advance();
                    self.dispatch_func(move |tx| {
                        let mut deflate = DeflateChunk::new(level, previous.clone(), current.clone());
                        deflate.run();
                        tx.send(ThreadMessage::DeflateDone(Arc::new(deflate))).unwrap();
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
                    self.dispatch_func(move |tx| {
                        let mut filter = FilterChunk::new(previous.clone(), current.clone());
                        filter.run();
                        tx.send(ThreadMessage::FilterDone(Arc::new(filter))).unwrap();
                    });
                },
                None => {
                    break;
                }
            }
        }

        Ok(())
    }

    pub fn append_row(&mut self, row: &[u8]) -> IoResult {
        if self.pixel_index >= self.chunks_total {
            // @todo use Err
            panic!("Tried to append beyond end of image");
        }

        Arc::get_mut(&mut self.pixel_accumulator).unwrap().append_row(row);

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
        Ok(())
    }

    pub fn progress(&self) -> f64 {
        self.chunks_output as f64 / self.chunks_total as f64
    }

    pub fn is_finished(&self) -> bool {
        self.chunks_output == self.chunks_total
    }

    pub fn flush(&mut self) -> IoResult {
        while self.chunks_output < self.pixel_index {
            // Dispatch any available async tasks and output.
            self.dispatch(DispatchMode::Blocking)?;
        }
        Ok(())
    }
}
