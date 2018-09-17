//
// mtpng - a multithreaded parallel PNG encoder in Rust
// lib.rs - library crate main file & public structs
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
#[macro_use] extern crate itertools;

// @fixme use a feature flag or?
extern crate libc;
mod capi;
pub use capi::*;

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
use Mode::Adaptive;

#[derive(Copy, Clone)]
#[repr(u8)]
pub enum ColorType {
    Greyscale = 0,
    Truecolor = 2,
    IndexedColor = 3,
    GreyscaleAlpha = 4,
    TruecolorAlpha = 6,
}
use ColorType::*;

impl ColorType {
    //
    // Todo: use TryFrom trait when it's stable.
    //
    pub fn try_from_u8(val: u8) -> io::Result<ColorType> {
        return match val {
            0 => Ok(ColorType::Greyscale),
            2 => Ok(ColorType::Truecolor),
            3 => Ok(ColorType::IndexedColor),
            4 => Ok(ColorType::GreyscaleAlpha),
            6 => Ok(ColorType::TruecolorAlpha),
            _ => Err(other("Inalid color type value")),
        }
    }

    pub fn is_depth_valid(&self, depth: u8) -> bool {
        match *self {
            Greyscale => match depth {
                1 | 2 | 4 | 8 | 16 => true,
                _ => false,
            },
            GreyscaleAlpha | Truecolor | TruecolorAlpha => match depth {
                8 | 16 => true,
                _ => false,
            },
            IndexedColor => match depth {
                1 | 2 | 4 | 8 => true,
                _ => false,
            },
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

    pub fn default() -> Header {
        Header::with_color(0, 0, ColorType::TruecolorAlpha)
    }

    pub fn channels(&self) -> usize {
        match self.color_type {
            ColorType::Greyscale => 1,
            ColorType::Truecolor => 3,
            ColorType::IndexedColor => 1,
            ColorType::GreyscaleAlpha => 2,
            ColorType::TruecolorAlpha => 4,
        }
    }

    //
    // warning this is specific to the filtering?
    // rename maybe
    //
    pub fn bytes_per_pixel(&self) -> usize {
        self.channels() * if self.depth > 8 {
            2
        } else {
            1
        }
    }

    //
    // Get the stride in bytes for the encoded pixel rows
    // matching the settings in this header.
    //
    // Will panic on arithmetic overflow.
    //
    pub fn stride(&self) -> usize {
        let bits_per_pixel = self.channels() * self.depth as usize;

        // Very long line lengths can overflow usize on 32-bit.
        // If we got this far, let it panic in the unwrap().
        let stride_bits = bits_per_pixel.checked_mul(self.width as usize)
                                        .unwrap();

        // And round up to nearest byte.
        let stride_bytes = stride_bits >> 3;
        let remainder = stride_bits & 3;
        if remainder > 0 {
            stride_bytes + 1
        } else {
            stride_bytes
        }
    }
}

#[derive(Copy, Clone)]
pub enum CompressionLevel {
    Fast,
    Default,
    High
}

#[derive(Copy, Clone)]
struct Options {
    chunk_size: usize,
    compression_level: CompressionLevel,
    strategy_mode: Mode<Strategy>,
    filter_mode: Mode<Filter>,
    streaming: bool,
}

impl Options {
    // Use default options
    pub fn new() -> Options {
        Options {
            chunk_size: 128 * 1024,
            compression_level: CompressionLevel::Default,
            strategy_mode: Adaptive,
            filter_mode: Adaptive,
            streaming: true,
        }
    }
}
