use std::io;
use std::io::{Error, ErrorKind};
use std::io::Write;

use std::mem;

use std::ptr;

use std::os::raw::*;

use ::libz_sys::*;

type IoResult = io::Result<()>;

fn invalid_input(payload: &str) -> Error
{
    Error::new(ErrorKind::InvalidInput, payload)
}

fn other(payload: &str) -> Error
{
    Error::new(ErrorKind::Other, payload)
}

unsafe fn char_ptr(byte_ref: &u8) -> *mut u8 {
    mem::transmute::<*const u8, *mut c_uchar>(byte_ref)
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

    pub fn set_level(mut self, level: u32) -> OptionsBuilder {
        self.options.level = level as c_int;
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

pub struct Deflate<W: Write> {
    output: W,
    options: Options,
    initialized: bool,
    stream: z_stream,
}

impl<W: Write> Deflate<W> {
    pub fn new(options: Options, w: W) -> Deflate<W> {
        Deflate {
            output: w,
            options: options,
            initialized: false,
            stream: unsafe {
                mem::zeroed()
            },
        }
    }

    pub fn init(&mut self) -> IoResult {
        if self.initialized {
            Ok(())
        } else {
            let ret = unsafe {
                deflateInit2_(&mut self.stream,
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
            deflateSetDictionary(&mut self.stream,
                                 &dict[0],
                                 dict.len() as c_uint)
        };
        match ret {
            Z_OK => Ok(()),
            Z_STREAM_ERROR => Err(invalid_input("Invalid parameter")),
            _ => Err(other("Unexpected error")),
        }
    }

    fn deflate(&mut self, data: &[u8], flush: Flush) -> IoResult {
        let buffer = [0u8; 32 * 1024];
        unsafe {
            self.stream.next_in = char_ptr(&data[0]);
            self.stream.avail_in = data.len() as c_uint;
        }
        loop {
            let ret = unsafe {
                self.stream.next_out = char_ptr(&buffer[0]);
                self.stream.avail_out = buffer.len() as c_uint;
                deflate(&mut self.stream, flush as c_int)
            };
            match ret {
                Z_OK | Z_STREAM_END => {
                    let end = buffer.len() - self.stream.avail_in as usize;
                    self.output.write(&buffer[0 .. end]);
                    if ret == Z_OK && self.stream.avail_in == 0 {
                        // Must call again; more output available.
                        continue;
                    } else {
                        return Ok(())
                    }
                },
                Z_STREAM_ERROR => return Err(invalid_input("Inconsistent stream state")),
                Z_BUF_ERROR => return Err(other("No progress possible")),
                _ => return Err(other("Unexpected error")),
            }
        }
    }

    pub fn write(&mut self, data: &[u8]) -> IoResult {
        self.init()?;
        self.deflate(data, Flush::NoFlush)
    }

    pub fn flush(&mut self, flush: Flush) -> IoResult {
        self.init()?;
        self.deflate(b"", flush)
    }

    pub fn finish(mut self, flush: Flush) -> io::Result<W> {
        self.flush(flush)?;
        let ret = unsafe {
            deflateEnd(&mut self.stream)
        };
        match ret {
            Z_OK => Ok(self.output),
            Z_STREAM_ERROR => Err(invalid_input("Inconsistent stream state")),
            Z_DATA_ERROR => Err(invalid_input("Stream freed early")),
            _ => Err(other("Unexpected error")),
        }
    }
}
