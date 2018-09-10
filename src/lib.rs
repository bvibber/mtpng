//
// mtpng - a multithreaded parallel PNG encoder in Rust
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

extern crate rayon;
extern crate crc;
extern crate libz_sys;

pub mod deflate;
pub mod filter;
pub mod encoder;
pub mod utils;
pub mod writer;

use std::io;

use deflate::Strategy;
use filter::Filter;
use utils::other;

//
// Like Option, but more specific. :D
//
#[derive(Copy, Clone)]
pub enum Mode<T> {
    Adaptive,
    Fixed(T),
}
use Mode::{Adaptive, Fixed};

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
    chunk_size: usize,
    compression_level: CompressionLevel,
    strategy: Strategy,
    streaming: bool,
    filter_mode: Mode<Filter>,
}

impl Options {
    // Use default options
    pub fn new() -> Options {
        Options {
            chunk_size: 128 * 1024,
            compression_level: CompressionLevel::Default,

            // @fixme make these depend on the input type!
            // Default works better for the none filter, which should be
            // used for indexed images in general...
            strategy: Strategy::Filtered,
            filter_mode: Adaptive,

            streaming: true,
        }
    }

    pub fn set_chunk_size(&mut self, chunk_size: usize) {
        self.chunk_size = chunk_size;
    }

    pub fn set_compression_level(&mut self, level: CompressionLevel) {
        self.compression_level = level;
    }

    pub fn set_filter_mode(&mut self, filter_mode: Mode<Filter>) {
        self.filter_mode = filter_mode;
    }

    pub fn set_strategy(&mut self, strategy: Strategy) {
        self.strategy = strategy;
    }

    pub fn set_streaming(&mut self, streaming: bool) {
        self.streaming = streaming;
    }
}

//
// Republish the Encoder type!
//
pub type Encoder<'a, W> = encoder::Encoder<'a, W>;

