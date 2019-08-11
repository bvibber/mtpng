//
// mtpng - a multithreaded parallel PNG encoder in Rust
// deflate.rs - wrapper for libz_sys suitable for making chunked deflate streams
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

use std::io;
use std::io::Write;

use std::mem;

use std::ptr;

use std::os::raw::*;

//use ::libz_sys::*;

use miniz_oxide::deflate;
use miniz_oxide::deflate::core::{
    CompressorOxide,
    compress_to_output,
    deflate_flags,
    TDEFLFlush,
    TDEFLStatus
};
use miniz_oxide::DataFormat;

use ::adler32::RollingAdler32;

use super::utils::*;

unsafe fn char_ptr(byte_ref: &u8) -> *mut u8 {
    mem::transmute::<*const u8, *mut c_uchar>(byte_ref)
}

pub fn adler32(sum: u32, bytes: &[u8]) -> u32 {
    let mut rolling = RollingAdler32::from_value(sum);
    rolling.update_buffer(bytes);
    rolling.hash()
}

pub fn adler32_initial() -> u32 {
    let buf: [u8; 0] = [];
    let rolling = RollingAdler32::from_buffer(&buf);
    rolling.hash()
}

pub fn adler32_combine(sum_a: u32, sum_b: u32, len_b: usize) -> u32 {
    // Ported from zlib
    // https://github.com/madler/zlib/blob/master/adler32.c
    const BASE: u32 = 65521;

    /* the derivation of this formula is left as an exercise for the reader */
    let len2: u32 = (len_b % BASE as usize) as u32;
    let rem: u32 = len2;
    let mut sum1: u32 = sum_a & 0xffff;
    let mut sum2: u32 = rem * sum1;
    sum2 %= BASE;
    sum1 += (sum_b & 0xffff) + BASE - 1;
    sum2 += ((sum_a >> 16) & 0xffff) + ((sum_b >> 16) & 0xffff) + BASE - rem;
    if sum1 >= BASE {
        sum1 -= BASE;
    }
    if sum1 >= BASE {
        sum1 -= BASE;
    }
    if sum2 >= (BASE << 1) {
        sum2 -= BASE << 1;
    }
    if sum2 >= BASE {
        sum2 -= BASE;
    }
    sum1 | (sum2 << 16)
}

#[derive(Copy, Clone)]
pub enum Strategy {
    Default,
    Filtered,
    HuffmanOnly,
    RLE,
    Fixed,
}

pub enum Method {
    Deflated
}

pub struct Options {
    level: c_int,
    method: Method,
    window_bits: c_int,
    mem_level: c_int,
    strategy: Strategy,
}

impl Options {
    pub fn new() -> Options {
        Options {
            level: -1, // Z_DEFAULT_COMPRESSION
            method: Method::Deflated,
            window_bits: 15,
            mem_level: 8,
            strategy: Strategy::Default,
        }
    }

    //
    // Compression level, 1 (fast) - 9 (high)
    //
    pub fn set_level(&mut self, level: i32) {
        self.level = level as c_int;
    }

    //
    // Default is 15 (32 KiB)
    // Set negative value for raw stream (no header/checksum)
    //
    pub fn set_window_bits(&mut self, bits: i32) {
        self.window_bits = bits as c_int;
    }

    pub fn set_strategy(&mut self, strategy: Strategy) {
        self.strategy = strategy;
    }

    //
    // Tune the memory buffer sizes for zlib deflate compression.
    //
    // Default is 8 (128 KiB)
    // Maximum is 9 (256 KiB)
    //
    // Total memory usage for zlib is approx:
    // (1 << (windowBits+2)) +  (1 << (memLevel+9))
    //
    pub fn set_mem_level(&mut self, level: i32) {
        self.mem_level = level as c_int;
    }
}

#[derive(Copy, Clone)]
pub enum Flush {
    NoFlush,
    PartialFlush,
    SyncFlush,
    FullFlush,
    Finish,
    Block,
    Trees,
}

pub struct Deflate<W: Write> {
    output: W,
    options: Options,
    initialized: bool,
    finished: bool,
    compressor: CompressorOxide,
}

impl<W: Write> Deflate<W> {
    pub fn new(options: Options, w: W) -> Deflate<W> {
        Deflate {
            output: w,
            options: options,
            initialized: false,
            finished: false,
            compressor: CompressorOxide::new(deflate_flags::TDEFL_WRITE_ZLIB_HEADER),
        }
    }

    pub fn init(&mut self) -> IoResult {
        if self.initialized {
            Ok(())
        } else {
            if self.options.level > -1 {
                self.compressor.set_format_and_level(DataFormat::Zlib, self.options.level as u8);
            }
            self.initialized = true;
            Ok(())
        }
    }

    pub fn set_dictionary(&mut self, dict: &[u8]) -> IoResult {
        self.init()?;
        // miniz_oxide doesn't expose setting the dictionary
        // so we'll cheat!
        self.compressor.set_compression_level(deflate::CompressionLevel::NoCompression);
        match compress_to_output(
            &mut self.compressor,
            dict,
            TDEFLFlush::Sync,
            |_data: &[u8]| -> bool {
                true
            }
        ).0 {
            TDEFLStatus::Okay => {},
            TDEFLStatus::Done => {},
            TDEFLStatus::PutBufFailed => {
                return Err(other("PutBufFailed"));
            },
            TDEFLStatus::BadParam => {
                return Err(invalid_input("Invalid parameter"));
            },
        }
        self.compressor.set_compression_level_raw(self.options.level as u8);
        Ok(())
    }

    fn deflate(&mut self, data: &[u8], flush: Flush) -> IoResult {
        self.init()?;

        let compressor = &mut self.compressor;
        let output = &mut self.output;
        return match compress_to_output(
            compressor,
            data,
            match flush {
                Flush::NoFlush => TDEFLFlush::None,
                Flush::FullFlush => TDEFLFlush::Full,
                Flush::SyncFlush => TDEFLFlush::Sync,
                Flush::Finish => TDEFLFlush::Finish,
                _ => TDEFLFlush::None,
            },
            |bytes: &[u8]| -> bool {
                match output.write(bytes) {
                    Ok(_) => true,
                    Err(_) => false,
                }
            }
        ).0 {
            TDEFLStatus::Okay => Ok(()),
            TDEFLStatus::Done => Ok(()),
            TDEFLStatus::PutBufFailed => Err(other("PutBufFailed")),
            TDEFLStatus::BadParam => Err(invalid_input("Invalid parameter")),
        }
    }

    pub fn write(&mut self, data: &[u8], flush: Flush) -> IoResult {
        self.init()?;
        self.deflate(data, flush)
    }

    //
    // Deallocate the zlib state and return the writer.
    //
    pub fn finish(mut self) -> io::Result<W> {
        Ok(self.output)
    }

    //
    // Get the checksum so far.
    //
    pub fn get_adler32(&self) -> u32 {
        return self.compressor.adler32();
    }
}
