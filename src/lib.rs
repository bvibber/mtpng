// Experimental parallel PNG writer
// Brion Vibber 2018-09-02

extern crate rayon;
extern crate crc;

mod state;
mod filter;
mod writer;

use rayon::ThreadPool;

use std::cmp;

use std::io;
use std::io::Write;
type IoResult = io::Result<()>;

use state::State;

#[derive(Copy, Clone)]
#[repr(u8)]
pub enum ColorType {
    Greyscale = 0,
    Truecolor = 2,
    IndexedColor = 3,
    GreyscaleAlpha = 4,
    TruecolorAlpha = 5,
}

#[derive(Copy, Clone)]
#[repr(u8)]
pub enum CompressionMethod {
    Deflate = 0,
}

#[derive(Copy, Clone)]
#[repr(u8)]
pub enum FilterMethod {
    Standard = 0,
}

#[derive(Copy, Clone)]
#[repr(u8)]
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
    compression_method: CompressionMethod,
    filter_method: FilterMethod,
    interlace_method: InterlaceMethod,
}

impl Header {
    pub fn new(width: u32, height: u32, color_type: ColorType, depth: u8, interlace_method: InterlaceMethod) -> Header {
        Header {
            width: width,
            height: height,
            depth: depth,
            color_type: color_type,
            compression_method: CompressionMethod::Deflate,
            filter_method: FilterMethod::Standard,
            interlace_method: interlace_method,
        }
    }

    pub fn with_depth(width: u32, height: u32, color_type: ColorType, depth: u8) -> Header {
        Header::new(width, height, color_type, depth, InterlaceMethod::Standard)
    }

    pub fn with_color(width: u32, height: u32, color_type: ColorType) -> Header {
        Header::with_depth(width, height, color_type, 8)
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

    pub fn bytes_per_pixel(&self) -> usize {
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
        }
    }

    pub fn stride(&self) -> usize {
        self.bytes_per_pixel() * self.width as usize
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
    streaming: bool,
}

impl Options {
    // Use default options
    pub fn default() -> Options {
        Options {
            chunk_size: 128 * 1024,
            compression_level: CompressionLevel::Default,
            streaming: true,
        }
    }
}

//
// A parallelized PNG encoder.
// Very unfinished.
//
pub struct Encoder<'a, W: Write> {
    state: State<'a, W>,
}

impl<'a, W: Write> Encoder<'a, W> {
    //
    // Create a new encoder using default thread pool.
    // Consumes the Write target, but you can get it back via Encoder::close()
    //
    pub fn new(header: Header, options: Options, writer: W) -> Encoder<'static, W> {
        Encoder {
            state: State::new(header, options, writer, None)
        }
    }

    //
    // Create a new encoder state using given thread pool
    //
    pub fn with_thread_pool(header: Header, options: Options, writer: W, thread_pool: &'a ThreadPool) -> Encoder<'a, W> {
        Encoder {
            state: State::new(header, options, writer, Some(thread_pool))
        }
    }

    //
    // Flush output and return the Write sink for further manipulation.
    // Consumes the encoder instance.
    //
    pub fn close(mut this: Encoder<'a, W>) -> io::Result<W> {
        State::close(this.state)
    }

    //
    // Copy a row's pixel data into buffers for async compression.
    // Returns immediately after copying.
    //
    pub fn append_row(&mut self, row: &[u8]) -> IoResult {
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
    pub fn flush(&mut self) -> IoResult {
        self.state.flush()
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
        match {
            let header = Header::with_color(width,
                                            height,
                                            ColorType::Truecolor);
            let options = Options::default();
            let mut writer = Vec::<u8>::new();
            let mut encoder = Encoder::new(header, options, writer);
            func(&mut encoder);
            Encoder::close(encoder)
        } {
            Ok(writer) => {},
            Err(e) => assert!(false, "Error {}", e),
        }
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
