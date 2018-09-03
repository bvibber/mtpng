// Experimental parallel PNG writer
// Brion Vibber 2018-09-02

extern crate rayon;

use rayon::ThreadPool;

use std::cmp;

use std::collections::HashMap;

use std::io::Write;

use std::sync::Arc;
use std::sync::mpsc;
use std::sync::mpsc::{Sender, Receiver};

#[derive(Copy, Clone)]
pub enum ColorType {
    Greyscale = 0,
    Truecolor = 2,
    IndexedColor = 3,
    GreyscaleAlpha = 4,
    TruecolorAlpha = 5,
}

#[derive(Copy, Clone)]
pub enum FilterMethod {
    Standard = 0,
}

#[derive(Copy, Clone)]
pub enum InterlaceMethod {
    Standard = 0,
    Adam7 = 1,
}

#[derive(Copy, Clone)]
pub struct Header {
    width: u32,
    height: u32,
    depth: u8,
    color_type: ColorType,
    filter_method: FilterMethod,
    interlace_method: InterlaceMethod,
}

impl Header {
    pub fn new(width: u32, height: u32, depth: u8, color_type: ColorType, filter_method: FilterMethod, interlace_method: InterlaceMethod) -> Header {
        Header {
            width: width,
            height: height,
            depth: depth,
            color_type: color_type,
            filter_method: filter_method,
            interlace_method: interlace_method,
        }
    }

    pub fn with_depth(width: u32, height: u32, depth: u8, color_type: ColorType) -> Header {
        Header::new(width, height, depth, color_type, FilterMethod::Standard, InterlaceMethod::Standard)
    }

    pub fn with_color(width: u32, height: u32, color_type: ColorType) -> Header {
        Header::with_depth(width, height, 8, color_type)
    }

    // @todo return errors gracefully
    pub fn validate(&self) -> bool {
        if self.width == 0 {
            panic!("Zero width");
        }
        if self.height == 0 {
            panic!("Zero height");
        }
        match self.color_type {
            ColorType::Greyscale => match self.depth {
                1 | 2 | 4 | 8 | 16 => {},
                _ => panic!("Invalid color depth"),
            },
            ColorType::Truecolor => match self.depth {
                8 | 16 => {},
                _ => panic!("Invalid color depth"),
            },
            ColorType::IndexedColor => match self.depth {
                1 | 2 | 4 | 8 => {},
                _ => panic!("Invalid color depth"),
            },
            ColorType::GreyscaleAlpha => match self.depth {
                8 | 16 => {},
                _ => panic!("Invalid color depth"),
            },
            ColorType::TruecolorAlpha => match self.depth {
                8 | 16 => {},
                _ => panic!("Invalid color depth"),
            }
        }
        match self.filter_method {
            FilterMethod::Standard => {},
        }
        match self.interlace_method {
            InterlaceMethod::Standard => {},
            InterlaceMethod::Adam7 => panic!("Interlacing not yet implemented."),
        }
        true
    }

    fn stride(&self) -> usize {
        return match self.color_type {
            ColorType::Greyscale => 1,
            ColorType::Truecolor => 3,
            ColorType::IndexedColor => 1,
            ColorType::GreyscaleAlpha => 2,
            ColorType::TruecolorAlpha => 4,
        } * if self.depth > 8 {
            2
        } else {
            1
        } * self.width as usize;
    }
}

#[derive(Copy, Clone)]
pub enum CompressionLevel {
    Fast,
    Default,
    High
}

#[derive(Copy, Clone)]
pub struct Options {
    chunk_size: usize,
    compression_level: CompressionLevel,
}

impl Options {
    // Use default options
    pub fn default() -> Options {
        Options {
            chunk_size: 128 * 1024,
            compression_level: CompressionLevel::Default,
        }
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
            panic!("Tried to access row from earlier chunk");
        } else if row >= self.end_row {
            panic!("Tried to access row from later chunk");
        } else {
            let start = self.stride * row;
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

    fn run(&mut self) {
        // -> run the filtering
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

struct State<'a, W: 'a + Write> {
    header: Header,
    options: Options,
    writer: &'a mut W,
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

impl<'a, W: 'a + Write> State<'a, W> {
    fn new(header: Header, options: Options, writer: &'a mut W, thread_pool: Option<&'a ThreadPool>) -> State<'a, W> {
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
                rayon::spawn(move || {
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

    fn dispatch(&mut self, mode: DispatchMode) {
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
                    self.writer.write(&current.data).unwrap(); // @fixme return error
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
    }

    fn append_row(&mut self, row: &[u8]) {
        if self.pixel_index >= self.chunks_total {
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
            self.dispatch(DispatchMode::NonBlocking);
        }
    }

    fn progress(&self) -> f64 {
        self.chunks_output as f64 / self.chunks_total as f64
    }

    fn is_finished(&self) -> bool {
        self.chunks_output == self.chunks_total
    }

    fn flush(&mut self) {
        while self.chunks_output < self.pixel_index {
            // Dispatch any available async tasks and output.
            self.dispatch(DispatchMode::Blocking);
        }
    }
}

//
// A parallelized PNG encoder.
// Very unfinished.
//
pub struct Encoder<'a, W: 'a + Write> {
    state: State<'a, W>,
}

impl<'a, W: 'a + Write> Encoder<'a, W> {
    //
    // Create a new encoder using default thread pool
    //
    pub fn new(header: Header, options: Options, writer: &'a mut W) -> Encoder<'a, W> {
        Encoder {
            state: State::new(header, options, writer, None)
        }
    }

    //
    // Create a new encoder state using given thread pool
    //
    pub fn with_thread_pool(header: Header, options: Options, writer: &'a mut W, thread_pool: &'a ThreadPool) -> Encoder<'a, W> {
        Encoder {
            state: State::new(header, options, writer, Some(thread_pool))
        }
    }

    //
    // Copy a row's pixel data into buffers for async compression.
    // Returns immediately after copying.
    //
    pub fn append_row(&mut self, row: &[u8]) {
        self.state.append_row(row)
    }

    //
    // Return completion progress as a fraction of 1.0
    //
    pub fn progress(&self) -> f64 {
        self.state.progress()
    }

    //
    // Return finished-ness state.
    // Is it finished? Yeah or no.
    //
    pub fn is_finished(&self) -> bool {
        self.state.is_finished()
    }

    //
    // Flush all currently in-progress data to output
    // Warning: this will block.
    //
    pub fn flush(&mut self) {
        self.state.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::Header;
    use super::ColorType;
    use super::Options;
    use super::Encoder;

    fn test_encoder<F>(width: u32, height: u32, func: F)
        where F: Fn(&mut Encoder<Vec<u8>>)
    {
        let header = Header::with_color(width,
                                        height,
                                        ColorType::Truecolor);
        let options = Options::default();
        let mut writer = Vec::<u8>::new();
        let mut encoder = Encoder::new(header, options, &mut writer);
        func(&mut encoder);
    }

    fn make_row(width: usize) -> Vec<u8> {
        let stride = width * 3;
        let mut row = Vec::<u8>::with_capacity(stride);
        for i in 0 .. stride {
            row.push((i % 255) as u8);
        }
        row
    }

    #[test]
    fn create_and_state() {
        test_encoder(7680, 2160, |encoder| {
            assert_eq!(encoder.is_finished(), false);
            assert_eq!(encoder.progress(), 0.0);
        });
    }

    #[test]
    fn test_one_row() {
        test_encoder(7680, 2160, |encoder| {
            let row = make_row(7680);
            encoder.append_row(&row);
            encoder.flush();

            // A single row should be not enough to trigger
            // a chunk.
            assert_eq!(encoder.is_finished(), false);
            assert_eq!(encoder.progress(), 0.0);
        });
    }

    #[test]
    fn test_many_rows() {
        test_encoder(7680, 2160, |encoder| {
            for _i in 0 .. 256 {
                let row = make_row(7680);
                encoder.append_row(&row);
            }
            encoder.flush();

            // Should trigger at least one block
            // but not enough to finish
            assert_eq!(encoder.is_finished(), false);
            assert!(encoder.progress() > 0.0);
        });
    }

    #[test]
    fn test_all_rows() {
        test_encoder(7680, 2160, |encoder| {
            for _i in 0 .. 2160 {
                let row = make_row(7680);
                encoder.append_row(&row);
            }
            encoder.flush();

            // Should trigger all blocks!
            assert_eq!(encoder.is_finished(), true);
            assert_eq!(encoder.progress(), 1.0);
        });
    }
}
