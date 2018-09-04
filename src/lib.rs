// Experimental parallel PNG writer
// Brion Vibber 2018-09-02

extern crate rayon;
extern crate crc;
extern crate libz_sys;

pub mod deflate;
pub mod filter;
pub mod encoder;
pub mod utils;
pub mod writer;

use rayon::ThreadPool;

use std::cmp;

use std::io;
use std::io::Write;
type IoResult = io::Result<()>;

use deflate::Strategy;
use filter::FilterMode;
use utils::other;

#[derive(Copy, Clone)]
#[repr(u8)]
pub enum ColorType {
    Greyscale = 0,
    Truecolor = 2,
    IndexedColor = 3,
    GreyscaleAlpha = 4,
    TruecolorAlpha = 6,
}

impl ColorType {
    pub fn from_u8(val: u8) -> io::Result<ColorType> {
        return match val {
            0 => Ok(ColorType::Greyscale),
            2 => Ok(ColorType::Truecolor),
            3 => Ok(ColorType::IndexedColor),
            4 => Ok(ColorType::GreyscaleAlpha),
            6 => Ok(ColorType::TruecolorAlpha),
            _ => Err(other("Inalid color type value")),
        }
    }
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
    // @fixme dont use pub?
    pub width: u32,
    pub height: u32,
    pub depth: u8,
    pub color_type: ColorType,
    pub compression_method: CompressionMethod,
    pub filter_method: FilterMethod,
    pub interlace_method: InterlaceMethod,
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
    pub chunk_size: usize,
    pub compression_level: CompressionLevel,
    pub strategy: Strategy,
    pub streaming: bool,
    pub filter_mode: FilterMode,
}

impl Options {
    // Use default options
    pub fn new() -> Options {
        Options {
            chunk_size: 128 * 1024,
            compression_level: CompressionLevel::Default,
            strategy: Strategy::Default,
            filter_mode: FilterMode::Adaptive,
            streaming: true,
        }
    }
}

//
// Republish the Encoder type!
//
pub type Encoder<'a, W> = encoder::Encoder<'a, W>;

