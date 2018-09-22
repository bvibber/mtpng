//
// mtpng - a multithreaded parallel PNG encoder in Rust
// utils.rs - misc bits
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
use std::io::{Error, ErrorKind, Write};

use std::mem;

use std::ptr;


pub type IoResult = io::Result<()>;

pub fn invalid_input(payload: &str) -> Error
{
    Error::new(ErrorKind::InvalidInput, payload)
}

pub fn other(payload: &str) -> Error
{
    Error::new(ErrorKind::Other, payload)
}

pub fn write_be32<W: Write>(w: &mut W, val: u32) -> IoResult {
    let bytes = [
        (val >> 24 & 0xff) as u8,
        (val >> 16 & 0xff) as u8,
        (val >> 8 & 0xff) as u8,
        (val & 0xff) as u8,
    ];
    w.write_all(&bytes)
}

pub fn write_be16<W: Write>(w: &mut W, val: u16) -> IoResult {
    let bytes = [
        (val >> 8 & 0xff) as u8,
        (val & 0xff) as u8,
    ];
    w.write_all(&bytes)
}

pub fn write_byte<W: Write>(w: &mut W, val: u8) -> IoResult {
    let bytes = [val];
    w.write_all(&bytes)
}

pub struct RowPool {
    row_len: usize,
    buffers: Vec<Vec<u8>>,
}

pub struct Row {
    data: Vec<u8>,
    row_pool: *mut RowPool,
}

impl Row {
    fn new(data: Vec<u8>, row_pool: &mut RowPool) -> Row {
        Row {
            data: data,
            row_pool: row_pool,
        }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    pub fn detach(mut self) -> Vec<u8> {
        // Swap out the buffer vector...
        let mut other = Vec::<u8>::new();
        mem::swap(&mut self.data, &mut other);

        // Clear the row_pool so we don't try to recycle the empty buffer
        self.row_pool = ptr::null_mut();

        other
    }
}

impl RowPool {
    pub fn new(row_len: usize) -> RowPool {
        RowPool {
            row_len: row_len,
            buffers: Vec::new(),
        }
    }

    pub fn claim(&mut self, len: usize) -> Row {
        assert!(len == self.row_len); // todo allow multiples or?

        let buf = match self.buffers.pop() {
            Some(buf) => buf,
            None => self.make_buffer()
        };
        Row::new(buf, self)
    }

    fn make_buffer(&self) -> Vec<u8> {
        vec![0; self.row_len]
    }

    pub fn recycle(&mut self, row: Row) {
        // The row's drop trait will call our recycle_buffer().
        drop(row);
    }

    fn recycle_buffer(&mut self, buf: Vec<u8>) {
        self.buffers.push(buf);
    }

    pub fn recycle_pool(&mut self, mut other: RowPool) {
        loop {
            match other.buffers.pop() {
                Some(buf) => self.recycle_buffer(buf),
                None => break,
            }
        }
    }
}

impl Drop for Row {
    fn drop(&mut self) {
        if !self.row_pool.is_null() {
            let mut other = Vec::<u8>::new();
            mem::swap(&mut self.data, &mut other);
            // Unsafe needed to hack into mutable.
            // DONT DO THIS.
            // EVER.
            unsafe {
                (*self.row_pool).recycle_buffer(other)
            }
        }
    }
}