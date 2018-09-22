//
// mtpng - a multithreaded parallel PNG encoder in Rust
// filter.rs - adaptive pixel filtering for PNG encoding
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

use std::cmp;
use std::io;
use std::cell::RefCell;

use typenum::Unsigned;
use typenum::consts::*;

use super::Header;
use super::Mode;
use super::Mode::{Adaptive, Fixed};

use super::utils::invalid_input;
use super::utils::{Row, RowPool};

#[repr(u8)]
#[derive(Copy, Clone)]
pub enum Filter {
    None = 0,
    Sub = 1,
    Up = 2,
    Average = 3,
    Paeth = 4,
}

impl Filter {
    //
    // Todo: use TryFrom trait when it's stable.
    //
    pub fn try_from_u8(val: u8) -> io::Result<Filter> {
        match val {
            0 => Ok(Filter::None),
            1 => Ok(Filter::Sub),
            2 => Ok(Filter::Up),
            3 => Ok(Filter::Average),
            4 => Ok(Filter::Paeth),
            _ => Err(invalid_input("Invalid filter constant value")),
        }
    }
}

//
// Using runtime bpp variable in the inner loop slows things down;
// specialize the filter functions for each possible constant size.
//
fn filter_iter_specialized<F>(bpp: usize, prev: &[u8], src: &[u8], out: &mut [u8], func: F)
    where F : Fn(u8, u8, u8, u8) -> u8 {
    match bpp {
        1 => filter_iter_generic::<F, U1>(prev, src, out, func), // indexed, greyscale@8
        2 => filter_iter_generic::<F, U2>(prev, src, out, func), // greyscale@16, greyscale+alpha@8
        3 => filter_iter_generic::<F, U3>(prev, src, out, func), // truecolor@8
        4 => filter_iter_generic::<F, U4>(prev, src, out, func), // truecolor@8, greyscale+alpha@16
        6 => filter_iter_generic::<F, U6>(prev, src, out, func), // truecolor@16
        8 => filter_iter_generic::<F, U8>(prev, src, out, func), // truecolor+alpha@16
        _ => panic!("Invalid bpp, should never happen."),
    }
}

//
// Iterator helper for the filter functions.
//
// Filters are all byte-wise, and accept four input pixels:
// val (current pixel), left, above, and upper_left.
//
// They return an offset value which is used to reconstruct
// the original pixels on decode based on the pixels decoded
// so far plus the offset.
//
#[inline(always)]
fn filter_iter_generic<F, BPP: Unsigned>(prev: &[u8], src: &[u8], out: &mut [u8], func: F)
    where F : Fn(u8, u8, u8, u8) -> u8
{
    assert!(src.len() == prev.len());
    assert!(src.len() == out.len());

    //
    // The izip! macro merges multiple iterators together.
    // Performs _slightly_ better than a for loop with indexing
    // and the bounds checks mostly factored out by careful
    // optimization, and doesn't require the voodoo assertions.
    //

    for (dest, cur, up) in
        izip!(&mut out[0 .. BPP::USIZE],
              &src[0 .. BPP::USIZE],
              &prev[0 .. BPP::USIZE]) {
        *dest = func(*cur, 0, *up, 0);
    }

    let len = out.len();
    for (dest, cur, left, up, above_left) in
        izip!(&mut out[BPP::USIZE .. len],
              &src[BPP::USIZE .. len],
              &src[0 .. len - BPP::USIZE],
              &prev[BPP::USIZE .. len],
              &prev[0 .. len - BPP::USIZE]) {
        *dest = func(*cur, *left, *up, *above_left);
    }
}

//
// "None" filter copies the untouched source data.
// Good for indexed color where there's no relation between pixel values.
//
// https://www.w3.org/TR/PNG/#9Filter-types
//
fn filter_none(_bpp: usize, _prev: &[u8], src: &[u8], dest: &mut [u8]) {
    // Does not need specialization.
    dest[0] = Filter::None as u8;
    dest[1 ..].clone_from_slice(src);
}

//
// "Sub" filter diffs each byte against its neighbor one pixel to the left.
// Good for lines that smoothly vary, like horizontal gradients.
//
// https://www.w3.org/TR/PNG/#9Filter-types
//
fn filter_sub(bpp: usize, prev: &[u8], src: &[u8], dest: &mut [u8]) {
    dest[0] = Filter::Sub as u8;

    filter_iter_specialized(bpp, &prev, &src, &mut dest[1 ..], |val, left, _above, _upper_left| -> u8 {
        val.wrapping_sub(left)
    })
}

//
// "Up" filter diffs the pixel against its upper neighbor from prev row.
// Good for vertical gradients and lines that are similar to their
// predecessors.
//
// https://www.w3.org/TR/PNG/#9Filter-types
//
fn filter_up(bpp: usize, prev: &[u8], src: &[u8], dest: &mut [u8]) {
    // Does not need specialization.
    dest[0] = Filter::Up as u8;

    filter_iter_specialized(bpp, &prev, &src, &mut dest[1 ..], |val, _left, above, _upper_left| -> u8 {
        val.wrapping_sub(above)
    })
}

//
// "Average" filter diffs the pixel against the average of its left and
// upper neighbors. Good for smoothly varying tonal and photographic images.
//
// https://www.w3.org/TR/PNG/#9Filter-type-3-Average
//
fn filter_average(bpp: usize, prev: &[u8], src: &[u8], dest: &mut [u8]) {
    dest[0] = Filter::Average as u8;

    filter_iter_specialized(bpp, &prev, &src, &mut dest[1 ..], |val, left, above, _upper_left| -> u8 {
        let avg = ((left as u32 + above as u32) / 2) as u8;
        val.wrapping_sub(avg)
    })
}

//
// Predictor function for the "Paeth" filter.
// The order of comparisons is important; use the PNG standard's reference.
//
// https://www.w3.org/TR/PNG/#9Filter-type-4-Paeth
//
#[inline(always)]
fn paeth_predictor(left: u8, above: u8, upper_left: u8) -> u8 {
    let a = left as i32;
    let b = above as i32;
    let c = upper_left as i32;

    let p = a + b - c;        // initial estimate
    let pa = i32::abs(p - a); // distances to a, b, c
    let pb = i32::abs(p - b);
    let pc = i32::abs(p - c);
    // return nearest of a,b,c,
    // breaking ties in order a,b,c.
    if pa <= pb && pa <= pc {
        left
    } else if pb <= pc {
        above
    } else {
        upper_left
    }
}

//
// The "Paeth" filter diffs each byte against the nearest one of its
// neighbor pixels, to the left, above, and upper-left.
//
// Good for photographic images and such.
//
// Note this is the most expensive filter to calculate.
//
// https://www.w3.org/TR/PNG/#9Filter-type-4-Paeth
//
fn filter_paeth(bpp: usize, prev: &[u8], src: &[u8], dest: &mut [u8]) {
    dest[0] = Filter::Paeth as u8;

    filter_iter_specialized(bpp, &prev, &src, &mut dest[1 ..], |val, left, above, upper_left| -> u8 {
        val.wrapping_sub(paeth_predictor(left, above, upper_left))
    })
}


//
// For the complexity/compressibility heuristic. Absolute value
// of the byte treated as a signed value, extended to a u32.
//
// Note this doesn't produce useful results on the "none" filter,
// as it's expecting, well, a filter delta. :D
//
fn filter_complexity_delta(val: u8) -> u32 {
    i32::abs(val as i8 as i32) as u32
}

//
// The maximum complexity heuristic value that can be represented
// without overflow.
//
fn complexity_max() -> u32 {
    u32::max_value() - 256
}

//
// Any row with this number of bytes needs to check for overflow
// of the complexity heuristic.
//
fn complexity_big_row(len: usize) -> bool {
    len >= (1 << 24)
}

//
// Complexity/compressibility heuristic recommended by the PNG spec
// and used in libpng as well.
//
// Note this doesn't produce useful results on the "none" filter
//
// libpng tries to do this inline with the filter with a clever
// early return if "too complex", but I find that's slower on large
// files than just running the whole filter.
//
fn estimate_complexity(data: &[u8]) -> u32 {
    let mut sum = 0u32;

    //
    // Very long rows could overflow the 32-bit complexity heuristic's
    // accumulator, but it doesn't trigger until tens of millions
    // of bytes per row. :)
    //
    // The check slows down the inner loop on more realistic sizes
    // (say, ~31k bytes for a 7680 wide RGBA image) so we skip it.
    //
    if complexity_big_row(data.len()) {
        for iter in data.iter() {
            sum = sum + filter_complexity_delta(*iter);
            if sum > complexity_max() {
                return complexity_max();
            }
        }
    } else {
        for iter in data.iter() {
            sum = sum + filter_complexity_delta(*iter);
        }
    }

    sum
}

fn filter_run(header: &Header,
              row_pool: &mut RowPool,
              mode: Filter,
              prev: &[u8],
              src: &[u8])
-> (Row, u32)
{
    let bpp = header.bytes_per_pixel();
    let stride = header.stride();
    let mut row = row_pool.claim(stride + 1);

    match mode {
        Filter::None => filter_none(bpp, prev, src, row.data_mut()),
        Filter::Sub => filter_sub(bpp, prev, src, row.data_mut()),
        Filter::Up => filter_up(bpp, prev, src, row.data_mut()),
        Filter::Average => filter_average(bpp, prev, src, row.data_mut()),
        Filter::Paeth => filter_paeth(bpp, prev, src, row.data_mut()),
    }

    let complexity = estimate_complexity(&row.data()[1..]);
    (row, complexity)
}

fn filter_fixed(header: &Header,
                row_pool: &mut RowPool,
                mode: Filter,
                prev: &[u8],
                src: &[u8])
-> Row
{
    filter_run(header, row_pool, mode, prev, src).0
}

fn filter_adaptive(header: &Header,
                   row_pool: &mut RowPool,
                   prev: &[u8],
                   src: &[u8])
-> Row
{
    //
    // Note the "none" filter is often good for things like
    // line-art diagrams and screenshots that have lots of
    // sharp pixel edges and long runs of solid colors.
    //
    // The adaptive filter algorithm doesn't work on it, however,
    // since it measures accumulated filter prediction offets and
    // that gives useless results on absolute color magnitudes.
    //
    // Compression could be improved for some files if a heuristic
    // can be devised to check if the none filter will work well.
    //

    let (data_sub, complexity_sub) = filter_run(header, row_pool, Filter::Sub, prev, src);
    let mut min = complexity_sub;

    let (data_up, complexity_up) = filter_run(header, row_pool, Filter::Up, prev, src);
    min = cmp::min(min, complexity_up);

    let (data_avg, complexity_avg) = filter_run(header, row_pool, Filter::Average, prev, src);
    min = cmp::min(min, complexity_avg);

    let (data_paeth, complexity_paeth) = filter_run(header, row_pool, Filter::Paeth, prev, src);
    min = cmp::min(min, complexity_paeth);

    if min == complexity_paeth {
        data_paeth
    } else if min == complexity_avg {
        data_avg
    } else if min == complexity_up {
        data_up
    } else /*if min == self.filter_sub.get_complexity() */ {
        data_sub
    }
}

pub fn filter(header: &Header,
              row_pool: &mut RowPool,
              mode: Mode<Filter>,
              prev: &[u8],
              src: &[u8])
-> Row
{
    match mode {
        Fixed(mode) => filter_fixed(header, row_pool, mode, prev, src),
        Adaptive    => filter_adaptive(header, row_pool, prev, src),
    }
}

#[cfg(test)]
mod tests {
    use super::filter;
    use super::Mode;
    use super::super::Header;
    use super::super::ColorType;
    use super::super::utils::RowPool;

    #[test]
    fn it_works() {
        let header = Header::with_color(1024, 768, ColorType::Truecolor);
        let mut row_pool = RowPool::new(1024 + 1);

        let prev = vec![0u8; header.stride()];
        let row = vec![0u8; header.stride()];
        let filtered_row = filter(&header, &mut row_pool, Mode::Adaptive, &prev, &row);
        assert_eq!(filtered_row.data().len(), header.stride() + 1);
    }

    #[test]
    fn it_works_16() {
        let header = Header::with_depth(1024, 768, ColorType::Truecolor, 16);
        let mut row_pool = RowPool::new(1024 + 1);

        let prev = vec![0u8; header.stride()];
        let row = vec![0u8; header.stride()];
        let filtered_row = filter(&header, &mut row_pool, Mode::Adaptive, &prev, &row);
        assert_eq!(filtered_row.data().len(), header.stride() + 1);
    }
}
