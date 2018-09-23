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

//! mtpng - a multithreaded parallel PNG encoder in Rust

extern crate rayon;
extern crate crc;
extern crate libz_sys;
#[macro_use] extern crate itertools;
extern crate typenum;

#[cfg(feature="capi")]
extern crate libc;
#[cfg(feature="capi")]
mod capi;
#[cfg(feature="capi")]
pub use capi::*;

pub mod deflate;
pub mod filter;
pub mod encoder;
pub mod utils;
pub mod writer;

use std::io;

use utils::{invalid_input, other};

/// Wrapper for filter and compression modes.
///
/// "Adaptive" means automatic selection based on content;
/// "Fixed" carries a specific mode.
#[derive(Copy, Clone)]
pub enum Mode<T> {
    Adaptive,
    Fixed(T),
}

/// PNG color types.
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
    /// Validate and produce a ColorType from one of the PNG header constants.
    //
    // Todo: use TryFrom trait when it's stable.
    //
    pub fn try_from_u8(val: u8) -> io::Result<ColorType> {
        return match val {
            0 => Ok(Greyscale),
            2 => Ok(Truecolor),
            3 => Ok(IndexedColor),
            4 => Ok(GreyscaleAlpha),
            6 => Ok(TruecolorAlpha),
            _ => Err(other("Inalid color type value")),
        }
    }

    /// Check if the given bit depth is valid for this color type.
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

/// PNG header compression method representation.
///
/// There is only one method defined, which is Deflate.
#[derive(Copy, Clone)]
#[repr(u8)]
pub enum CompressionMethod {
    Deflate = 0,
}

/// PNG header filter method representation.
///
/// Currently only Standard is supported. This may be expanded to support APNG in future.
#[derive(Copy, Clone)]
#[repr(u8)]
pub enum FilterMethod {
    Standard = 0,
}

/// PNG header interlace method representation.
///
/// Currently only Standard is supported; Adam7 interlacing will throw an error if used.
#[derive(Copy, Clone)]
#[repr(u8)]
pub enum InterlaceMethod {
    Standard = 0,
    Adam7 = 1,
}

/// PNG header representation.
///
/// You must create one of these with image metadata when encoding,
/// and can reuse the header for multiple encodings if desired.
#[derive(Copy, Clone)]
pub struct Header {
    pub width: u32,
    pub height: u32,
    pub depth: u8,
    pub color_type: ColorType,
    pub compression_method: CompressionMethod,
    pub filter_method: FilterMethod,
    pub interlace_method: InterlaceMethod,
}

impl Header {
    /// Create a new Header struct with default settings.
    ///
    /// This will be 1x1 pixels, TruecolorAlpha 8-bit, with
    /// the standard compression, filter, and interlace methods.
    /// You can mutate the state using the set_* methods.
    pub fn new() -> Header {
        Header {
            width: 1,
            height: 1,
            depth: 8,
            color_type: ColorType::TruecolorAlpha,
            compression_method: CompressionMethod::Deflate,
            filter_method: FilterMethod::Standard,
            interlace_method: InterlaceMethod::Standard,
        }
    }

    /// Get the number of channels per pixel for this image's color type.
    pub fn channels(&self) -> usize {
        match self.color_type {
            ColorType::Greyscale => 1,
            ColorType::Truecolor => 3,
            ColorType::IndexedColor => 1,
            ColorType::GreyscaleAlpha => 2,
            ColorType::TruecolorAlpha => 4,
        }
    }

    /// Get the bytes per pixel for this image, for PNG filtering purposes.
    ///
    /// If the bit depth is < 8, this will clamp at 1.
    pub fn bytes_per_pixel(&self) -> usize {
        self.channels() * if self.depth > 8 {
            2
        } else {
            1
        }
    }

    /// Get the stride in bytes for the encoded pixel rows matching the settings in this header.
    ///
    /// Will panic on arithmetic overflow if given pathologically long rows.
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

    /// Set the pixel dimensions of the image.
    ///
    /// Returns error if width or height are 0.
    ///
    /// Warning: it's possible to make combinations of width and color type
    /// that cannot fit in memory on 32-bit systems. These are not detected
    /// here, but will panic when stride() is called.
    pub fn set_size(&mut self, width: u32, height: u32) -> io::Result<()> {
        if width == 0 {
            Err(invalid_input("width cannot be 0"))
        } else if height == 0 {
            Err(invalid_input("height canno tbe 0"))
        } else {
            self.width = width;
            self.height = height;
            Ok(())
        }
    }

    /// Set the color type and depth of the image.
    ///
    /// Returns error if depth is invalid for the given color type.
    ///
    /// Warning: it's possible to make combinations of width and color type
    /// that cannot fit in memory on 32-bit systems. These are not detected
    /// here, but will panic when stride() is called.
    pub fn set_color(&mut self, color_type: ColorType, depth: u8) -> io::Result<()> {
        if !color_type.is_depth_valid(depth) {
            Err(invalid_input("invalid color depth for this color type"))
        } else {
            self.color_type = color_type;
            self.depth = depth;
            Ok(())
        }
    }
}

/// Representation of deflate compression level.
///
/// Default is zlib's 6; Fast is 1; High is 9.
#[derive(Copy, Clone)]
pub enum CompressionLevel {
    Fast,
    Default,
    High
}
