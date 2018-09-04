use std::io;
use std::io::{Error, ErrorKind};
use std::io::Write;

use std::mem;

use std::ptr;

use std::os::raw::*;

use ::libz_sys::*;

use super::utils::*;

unsafe fn char_ptr(byte_ref: &u8) -> *mut u8 {
    mem::transmute::<*const u8, *mut c_uchar>(byte_ref)
}

pub fn adler32(sum: u32, bytes: &[u8]) -> u32 {
    unsafe {
        ::libz_sys::adler32(sum as c_ulong, &bytes[0], bytes.len() as c_uint) as u32
    }
}

pub fn adler32_initial() -> u32 {
    unsafe {
        ::libz_sys::adler32(0, ptr::null(), 0) as u32
    }
}

pub fn adler32_combine(sum_a: u32, sum_b: u32, len_b: usize) -> u32 {
    unsafe {
        ::libz_sys::adler32_combine(sum_a as c_ulong, sum_b as c_ulong, len_b as c_long) as u32
    }
}

pub struct Options {
    level: c_int,
    method: c_int,
    window_bits: c_int,
    mem_level: c_int,
    strategy: c_int,
}

pub struct OptionsBuilder {
    options: Options,
}

impl OptionsBuilder {
    pub fn new() -> OptionsBuilder {
        OptionsBuilder {
            options: Options {
                level: Z_DEFAULT_COMPRESSION,
                method: Z_DEFLATED,
                window_bits: 15,
                mem_level: 8,
                strategy: Z_DEFAULT_STRATEGY,
            }
        }
    }

    pub fn set_level(mut self, level: i32) -> OptionsBuilder {
        self.options.level = level as c_int;
        self
    }

    //
    // Default is 15 (32 KiB)
    // Set negative value for raw stream (no header/checksum)
    //
    pub fn set_window_bits(mut self, bits: i32) -> OptionsBuilder {
        self.options.window_bits = bits as c_int;
        self
    }

    pub fn finish(mut self) -> Options {
        self.options
    }
}

#[derive(Copy, Clone)]
pub enum Flush {
    NoFlush = Z_NO_FLUSH as isize,
    PartialFlush = Z_PARTIAL_FLUSH as isize,
    SyncFlush = Z_SYNC_FLUSH as isize,
    FullFlush = Z_FULL_FLUSH as isize,
    Finish = Z_FINISH as isize,
    Block = Z_BLOCK as isize,
    Trees = Z_TREES as isize,
}

enum Output {
    Write,
    Discard,
}

pub struct Deflate<W: Write> {
    output: W,
    options: Options,
    initialized: bool,
    finished: bool,
    stream: Box<z_stream>,
}

impl<W: Write> Deflate<W> {
    pub fn new(options: Options, w: W) -> Deflate<W> {
        Deflate {
            output: w,
            options: options,
            initialized: false,
            finished: false,
            stream: Box::new(unsafe {
                mem::zeroed()
            }),
        }
    }

    pub fn init(&mut self) -> IoResult {
        if self.initialized {
            Ok(())
        } else {
            let ret = unsafe {
                deflateInit2_(&mut *self.stream,
                              self.options.level,
                              self.options.method,
                              self.options.window_bits,
                              self.options.mem_level,
                              self.options.strategy,
                              zlibVersion(),
                              mem::size_of::<z_stream>() as c_int)
            };
            return match ret {
                Z_OK => {
                    self.initialized = true;
                    Ok(())
                },
                Z_MEM_ERROR => Err(other("Out of memory")),
                Z_STREAM_ERROR => Err(invalid_input("Invalid parameter")),
                Z_VERSION_ERROR => Err(invalid_input("Incompatible version of zlib")),
                _ => Err(other("Unexpected error")),
            }
        }
    }

    pub fn set_dictionary(&mut self, dict: &[u8]) -> IoResult {
        self.init()?;
        let ret = unsafe {
            deflateSetDictionary(&mut *self.stream,
                                 &dict[0],
                                 dict.len() as c_uint)
        };
        match ret {
            Z_OK => Ok(()),
            Z_STREAM_ERROR => Err(invalid_input("Invalid parameter")),
            _ => Err(other("Unexpected error")),
        }
    }

    fn deflate(&mut self, data: &[u8], flush: Flush, output: Output) -> IoResult {
        self.init()?;
        let buffer = [0u8; 128 * 1024];
        let mut stream = &mut *self.stream;
        unsafe {
            stream.next_in = char_ptr(&data[0]);
            stream.avail_in = data.len() as c_uint;
        }
        loop {
            let ret = unsafe {
                stream.next_out = char_ptr(&buffer[0]);
                stream.avail_out = buffer.len() as c_uint;

                deflate(stream, flush as c_int)
            };
            match ret {
                Z_OK | Z_STREAM_END => {
                    match output {
                        Output::Write => {
                            let end = buffer.len() - stream.avail_out as usize;
                            self.output.write_all(&buffer[0 .. end])?;
                        },
                        Output::Discard => {
                            // ignore it
                        },
                    }
                    match ret {
                        Z_OK => {
                            if stream.avail_out == 0 {
                                // Must call again; more output available.
                                continue;
                            } else {
                                return Ok(());
                            }
                        },
                        Z_STREAM_END => {
                            self.finished = true;
                            if stream.avail_out == 0 {
                                // Must call again; more output available.
                                continue;
                            } else {
                                return Ok(());
                            }
                        },
                        _ => unreachable!(),
                    }
                },
                Z_STREAM_ERROR => return Err(invalid_input("Inconsistent stream state")),
                Z_BUF_ERROR => return Err(other("No progress possible")),
                _ => return Err(other("Unexpected error")),
            }
        }
    }

    pub fn write(&mut self, data: &[u8], flush: Flush) -> IoResult {
        self.init()?;
        self.deflate(data, flush, Output::Write)
    }

    //
    // Deallocate the zlib state and return the writer.
    //
    pub fn finish(mut self) -> io::Result<W> {
        return if self.initialized {
            let ret = unsafe {
                deflateEnd(&mut *self.stream)
            };
            match ret {
                // Z_DATA_ERROR means we freed before finishing the stream.
                // For our use case we do this deliberately, it's ok!
                Z_OK | Z_DATA_ERROR => Ok(self.output),
                Z_STREAM_ERROR => Err(invalid_input("Inconsistent stream state")),
                _ => Err(other("Unexpected error")),
            }
        } else {
            Ok(self.output)
        }
    }

    //
    // Get the checksum so far.
    //
    pub fn get_adler32(&self) -> u32 {
        (*self.stream).adler as u32
    }
}
