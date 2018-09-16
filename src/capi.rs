//
// mtpng - a multithreaded parallel PNG encoder in Rust
// capi.rs - C API implementation
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

use rayon::ThreadPool;
use rayon::ThreadPoolBuilder;

use std::io;
use std::io::Read;
use std::io::Write;

use std::ptr;

use libc::{c_void, c_int, size_t, uint8_t, uint32_t};

use super::ColorType;
use super::CompressionLevel;
use super::Mode;
use super::Mode::{Adaptive, Fixed};

use super::encoder::Encoder;

use super::filter::Filter;

use super::deflate::Strategy;

use super::utils::invalid_input;
use super::utils::other;

#[repr(C)]
pub enum CResult {
    Ok = 0,
    Err = 1,
}

impl From<Result<(),io::Error>> for CResult {
    fn from(result: Result<(),io::Error>) -> CResult {
        match result {
            Ok(()) => CResult::Ok,
            Err(_) => CResult::Err,
        }
    }
}

pub type CReadFunc = unsafe extern "C"
    fn(*const c_void, *mut uint8_t, size_t) -> size_t;

pub type CWriteFunc = unsafe extern "C"
    fn(*const c_void, *const uint8_t, size_t) -> size_t;

pub type CFlushFunc = unsafe extern "C"
    fn(*const c_void) -> bool;

//
// Adapter for Read trait to use C callback.
//
pub struct CReader {
    read_func: CReadFunc,
    user_data: *const c_void,
}

impl CReader {
    fn new(read_func: CReadFunc,
           user_data: *const c_void)
    -> CReader
    {
        CReader {
            read_func: read_func,
            user_data: user_data,
        }
    }
}

impl Read for CReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let ret = unsafe {
            (self.read_func)(self.user_data,
                             &mut buf[0],
                             buf.len())
        };
        if ret == buf.len() {
            Ok(ret)
        } else {
            Err(other("mtpng read callback returned failure"))
        }
    }
}

//
// Adapter for Write trait to use C callbacks.
//
pub struct CWriter {
    write_func: CWriteFunc,
    flush_func: CFlushFunc,
    user_data: *const c_void,
}

impl CWriter {
    fn new(write_func: CWriteFunc,
           flush_func: CFlushFunc,
           user_data: *const c_void)
    -> CWriter
    {
        CWriter {
            write_func: write_func,
            flush_func: flush_func,
            user_data: user_data,
        }
    }
}

impl Write for CWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let ret = unsafe {
            (self.write_func)(self.user_data,
                                   &buf[0],
                                   buf.len())
        };
        if ret == buf.len() {
            Ok(ret)
        } else {
            Err(other("mtpng write callback returned failure"))
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        let ret = unsafe {
            (self.flush_func)(self.user_data)
        };
        if ret {
            Ok(())
        } else {
            Err(other("mtpng flush callback returned failure"))
        }
    }
}

// Cheat on the lifetimes?
type CEncoder = Encoder<'static, CWriter>;

pub type PThreadPool = *mut ThreadPool;
pub type PEncoder = *mut CEncoder;


#[no_mangle]
pub unsafe extern "C"
fn mtpng_threadpool_new(pp_pool: *mut PThreadPool, threads: size_t)
-> CResult
{
    CResult::from(|| -> io::Result<()> {
        if pp_pool.is_null() {
            Err(invalid_input("input pointer must not be null"))
        } else {
            let pool = ThreadPoolBuilder::new().num_threads(threads)
                                               .build()
                                               .map_err(|err| other(&err.to_string()))?;
            *pp_pool = Box::into_raw(Box::new(pool));
            Ok(())
        }
    }())
}

#[no_mangle]
pub unsafe extern "C"
fn mtpng_threadpool_release(pp_pool: *mut PThreadPool)
-> CResult
{
    CResult::from(|| -> io::Result<()> {
        if pp_pool.is_null() {
            Err(invalid_input("Pointer must not be null"))
        } else {
            drop(Box::from_raw(*pp_pool));
            *pp_pool = ptr::null_mut();
            Ok(())
        }
    }())
}



#[no_mangle]
pub unsafe extern "C"
fn mtpng_encoder_new(pp_encoder: *mut PEncoder,
                     write_func: Option<CWriteFunc>,
                     flush_func: Option<CFlushFunc>,
                     user_data: *const c_void,
                     p_pool: PThreadPool)
-> CResult
{
    CResult::from(|| -> io::Result<()> {
        if pp_encoder.is_null() {
            Err(invalid_input("Pointer to pointer must not be null"))
        } else {
            match (write_func, flush_func) {
                (Some(wf), Some(ff)) => {
                    let writer = CWriter::new(wf, ff, user_data);
                    if p_pool.is_null() {
                        let encoder = Encoder::new(writer);
                        *pp_encoder = Box::into_raw(Box::new(encoder));
                    } else {
                        let encoder = Encoder::with_thread_pool(writer, &*p_pool);
                        *pp_encoder = Box::into_raw(Box::new(encoder));
                    }
                    Ok(())
                },
                _ => {
                    Err(invalid_input("Callbacks must not be null"))
                }
            }
        }
    }())
}

#[no_mangle]
pub unsafe extern "C"
fn mtpng_encoder_release(pp_encoder: *mut PEncoder)
-> CResult
{
    CResult::from(|| -> io::Result<()> {
        if pp_encoder.is_null() {
            Err(invalid_input("Pointer to pointer must not be null"))
        } else {
            if (*pp_encoder).is_null() {
                Err(invalid_input("Pointer must not be null"))
            } else {
                drop(Box::from_raw(*pp_encoder));
                *pp_encoder = ptr::null_mut();
                Ok(())
            }
        }
    }())
}

#[no_mangle]
pub unsafe extern "C"
fn mtpng_encoder_set_size(p_encoder: PEncoder,
                          width: uint32_t,
                          height: uint32_t)
-> CResult
{
    CResult::from(|| -> io::Result<()> {
        if p_encoder.is_null() {
            Err(invalid_input("Pointer must not be null"))
        } else {
            (*p_encoder).set_size(width, height)
        }
    }())
}

#[no_mangle]
pub unsafe extern "C"
fn mtpng_encoder_set_color(p_encoder: PEncoder,
                           color_type: c_int,
                           depth: uint8_t)
-> CResult
{
    CResult::from(|| -> io::Result<()> {
        if p_encoder.is_null() {
            Err(invalid_input("Pointer must not be null"))
        } else if color_type < 0 || color_type > u8::max_value() as c_int {
            Err(invalid_input("Invalid color type"))
        } else {
            let color = ColorType::try_from_u8(color_type as u8)?;
            (*p_encoder).set_color(color, depth)
        }
    }())
}

#[no_mangle]
pub unsafe extern "C"
fn mtpng_encoder_set_filter(p_encoder: PEncoder,
                            filter_mode: c_int)
-> CResult
{
    CResult::from(|| -> io::Result<()> {
        if p_encoder.is_null() {
            Err(invalid_input("Pointer must not be null"))
        } else if filter_mode > u8::max_value() as c_int {
            Err(invalid_input("Invalid filter mode"))
        } else {
            let mode = if filter_mode < 0 {
                Adaptive
            } else {
                Fixed(Filter::try_from_u8(filter_mode as u8)?)
            };
            (*p_encoder).set_filter_mode(mode)
        }
    }())
}

#[no_mangle]
pub unsafe extern "C"
fn mtpng_encoder_set_chunk_size(p_encoder: PEncoder,
                                chunk_size: size_t)
-> CResult
{
    CResult::from(|| -> io::Result<()> {
        if p_encoder.is_null() {
            Err(invalid_input("Pointer must not be null"))
        } else {
            (*p_encoder).set_chunk_size(chunk_size)
        }
    }())
}

#[no_mangle]
pub unsafe extern "C"
fn mtpng_encoder_write_header(p_encoder: PEncoder)
-> CResult
{
    CResult::from(|| -> io::Result<()> {
        if p_encoder.is_null() {
            Err(invalid_input("Pointer must not be null"))
        } else {
            (*p_encoder).write_header()
        }
    }())
}

#[no_mangle]
pub unsafe extern "C"
fn mtpng_encoder_write_image(p_encoder: PEncoder,
                             read_func: Option<CReadFunc>,
                             user_data: *const c_void)
-> CResult
{
    CResult::from(|| -> io::Result<()> {
        if p_encoder.is_null() {
            Err(invalid_input("Pointer must not be null"))
        } else {
            match read_func {
                Some(rf) => {
                    let mut reader = CReader::new(rf, user_data);
                    (*p_encoder).write_image(&mut reader)
                },
                _ => {
                    Err(invalid_input("read_func must not be null"))
                }
            }
        }
    }())
}

#[no_mangle]
pub unsafe extern "C"
fn mtpng_encoder_write_image_rows(p_encoder: PEncoder,
                                  p_bytes: *const uint8_t,
                                  len: size_t)
-> CResult
{
    CResult::from(|| -> io::Result<()> {
        if p_encoder.is_null() {
            Err(invalid_input("p_encoder must not be null"))
        } else if p_bytes.is_null() {
            Err(invalid_input("p_bytes must not be null"))
        } else {
            let slice = ::std::slice::from_raw_parts(p_bytes, len);
            (*p_encoder).write_image_rows(slice)
        }
    }())
}

#[no_mangle]
pub unsafe extern "C"
fn mtpng_encoder_finish(pp_encoder: *mut PEncoder)
-> CResult
{
    CResult::from(|| -> io::Result<()> {
        if pp_encoder.is_null() {
            Err(invalid_input("pp_encoder must not be null"))
        } else if (*pp_encoder).is_null() {
            Err(invalid_input("*pp_encoder must not be null"))
        } else {
            let b_encoder = Box::from_raw(*pp_encoder);
            *pp_encoder = ptr::null_mut();
            b_encoder.finish()?;
            Ok(())
        }
    }())
}
