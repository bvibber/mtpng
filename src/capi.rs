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

use rayon::ThreadPoolBuilder;
use rayon::ThreadPool;

use std::io;
use std::io::Write;

use std::slice;
use std::ptr;

use libc::{c_void, c_int, size_t, uint8_t, uint32_t};

use super::ColorType;
use super::CompressionLevel;
use super::Mode;
use super::Mode::{Adaptive, Fixed};

use super::encoder::Encoder;

use super::filter::Filter;

use super::deflate::Strategy;

use super::utils::other;

#[repr(C)]
pub enum CResult {
    Ok = 0,
    Err = 1,
}

pub type CWriteFunc = unsafe extern "C"
    fn(*const c_void, *const uint8_t, size_t) -> size_t;

pub type CFlushFunc = unsafe extern "C"
    fn(*const c_void) -> bool;

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
    if pp_pool.is_null() {
        CResult::Err
    } else {
        match ThreadPoolBuilder::new().num_threads(threads).build() {
            Ok(pool) => {
                *pp_pool = Box::into_raw(Box::new(pool));
                CResult::Ok
            },
            Err(_err) => {
                CResult::Err
            }
        }
    }
}

#[no_mangle]
pub unsafe extern "C"
fn mtpng_threadpool_release(pp_pool: *mut PThreadPool)
-> CResult
{
    if pp_pool.is_null() {
        CResult::Err
    } else {
        drop(Box::from_raw(*pp_pool));
        *pp_pool = ptr::null_mut();
        CResult::Ok
    }
}



#[no_mangle]
pub unsafe extern "C"
fn mtpng_encoder_new(pp_encoder: *mut PEncoder,
                     write_func: CWriteFunc,
                     flush_func: CFlushFunc,
                     user_data: *const c_void,
                     p_pool: PThreadPool)
-> CResult
{
    if pp_encoder.is_null() {
        CResult::Err
    } else {
        let writer = CWriter::new(write_func, flush_func, user_data);
        if p_pool.is_null() {
            let encoder = Encoder::new(writer);
            *pp_encoder = Box::into_raw(Box::new(encoder));
            CResult::Ok
        } else {
            let encoder = Encoder::with_thread_pool(writer, &*p_pool);
            *pp_encoder = Box::into_raw(Box::new(encoder));
            CResult::Ok
        }
    }
}

#[no_mangle]
pub unsafe extern "C"
fn mtpng_encoder_release(pp_encoder: *mut PEncoder)
-> CResult
{
    if pp_encoder.is_null() {
        CResult::Err
    } else {
        if (*pp_encoder).is_null() {
            CResult::Err
        } else {
            drop(Box::from_raw(*pp_encoder));
            *pp_encoder = ptr::null_mut();
            CResult::Ok
        }
    }
}

#[no_mangle]
pub unsafe extern "C"
fn mtpng_encoder_set_size(p_encoder: PEncoder,
                          width: uint32_t,
                          height: uint32_t)
-> CResult
{
    match (*p_encoder).set_size(width, height) {
        Ok(()) => CResult::Ok,
        Err(_) => CResult::Err,
    }
}

#[no_mangle]
pub unsafe extern "C"
fn mtpng_encoder_set_color(p_encoder: PEncoder,
                           color_type: c_int,
                           depth: uint8_t)
-> CResult
{
    if color_type < 0 || color_type > u8::max_value() as c_int {
        CResult::Err
    } else {
        match ColorType::from_u8(color_type as u8) {
            Ok(color) => match (*p_encoder).set_color(color, depth) {
                Ok(()) => CResult::Ok,
                Err(_) => CResult::Err,
            },
            Err(_) => CResult::Err,
        }
    }
}

#[no_mangle]
pub unsafe extern "C"
fn mtpng_encoder_set_chunk_size(p_encoder: PEncoder,
                                chunk_size: size_t)
-> CResult
{
    match (*p_encoder).set_chunk_size(chunk_size) {
        Ok(()) => CResult::Ok,
        Err(_) => CResult::Err,
    }
}


#[no_mangle]
pub unsafe extern "C"
fn mtpng_encoder_write_header(p_encoder: PEncoder)
-> CResult
{
    match (*p_encoder).write_header() {
        Ok(()) => CResult::Ok,
        Err(_) => CResult::Err,
    }
}

#[no_mangle]
pub unsafe extern "C"
fn mtpng_encoder_append_row(p_encoder: PEncoder,
                            p_bytes: *const uint8_t,
                            len: size_t)
-> CResult
{
    if p_encoder.is_null() {
        CResult::Err
    } else if p_bytes.is_null() {
        CResult::Err
    } else {
        match (*p_encoder).append_row(slice::from_raw_parts(p_bytes, len)) {
            Ok(()) => CResult::Ok,
            Err(_) => CResult::Err,
        }
    }
}

#[no_mangle]
pub unsafe extern "C"
fn mtpng_encoder_finish(pp_encoder: *mut PEncoder)
-> CResult
{
    if pp_encoder.is_null() {
        CResult::Err
    } else if (*pp_encoder).is_null() {
        CResult::Err
    } else {
        let b_encoder = Box::from_raw(*pp_encoder);
        *pp_encoder = ptr::null_mut();
        match b_encoder.finish() {
            Ok(_writer) => CResult::Ok,
            Err(_) => CResult::Err,
        }
    }
}
