use std::cmp;
use std::io::Write;

use super::Header;
use super::ColorType;

#[repr(u8)]
pub enum FilterType {
    None = 0,
    Sub = 1,
    Up = 2,
    Average = 3,
    Paeth = 4,
}

fn filter_none(_bpp: usize, _prev: &[u8], src: &[u8], dest: &mut [u8]) {
    dest[0] = FilterType::None as u8;
    dest[1 ..].clone_from_slice(src);
}

fn filter_sub(bpp: usize, _prev: &[u8], src: &[u8], dest: &mut [u8]) {
    dest[0] = FilterType::Sub as u8;

    let out = &mut dest[1 ..];
    for i in 0 .. bpp {
        out[i] = src[i];
    }

    let len = src.len();
    for i in bpp .. len {
        out[i] = src[i].wrapping_sub(src[i - bpp]);
    }
}

fn filter_up(_bpp: usize, prev: &[u8], src: &[u8], dest: &mut [u8]) {
    dest[0] = FilterType::Up as u8;

    let out = &mut dest[1 ..];
    let len = src.len();
    for i in 0 .. len {
        out[i] = src[i].wrapping_sub(prev[i]);
    }
}

fn filter_average(bpp: usize, prev: &[u8], src: &[u8], dest: &mut [u8]) {
    dest[0] = FilterType::Average as u8;

    let out = &mut dest[1 ..];
    for i in 0 .. bpp {
        let above = prev[i];
        let avg = (above as u32 / 2) as u8;
        out[i] = src[i].wrapping_sub(avg);
    }

    let len = src.len();
    for i in bpp .. len {
        let left = src[i - bpp];
        let above = prev[i];
        let avg = ((left as u32 + above as u32) / 2) as u8;
        out[i] = src[i].wrapping_sub(avg);
    }
}

// From the PNG standard
fn paeth_predictor(left: u8, above: u8, upper_left: u8) -> u8 {
    let a = left as i32;
    let b = above as i32;
    let c = upper_left as i32;

    let p = a + b - c;   // initial estimate
    let pa = i32::abs(p - a); // distances to a, b, c
    let pb = i32::abs(p - c);
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

fn filter_paeth(bpp: usize, prev: &[u8], src: &[u8], dest: &mut [u8]) {
    dest[0] = FilterType::Paeth as u8;

    let out = &mut dest[1 ..];
    for i in 0 .. bpp {
        let left = 0;
        let above = prev[i];
        let upper_left = 0;
        let predict = paeth_predictor(left, above, upper_left);
        out[i] = src[i].wrapping_sub(predict);
    }

    let len = src.len();
    for i in bpp .. len {
        let left = src[i - bpp];
        let above = prev[i];
        let upper_left = prev[i - bpp];
        let predict = paeth_predictor(left, above, upper_left);
        out[i] = src[i].wrapping_sub(predict);
    }
}

fn estimate_complexity(filtered_row: &[u8]) -> i64 {
    let data = &filtered_row[1 ..];
    let len = data.len();
    let mut sum = 0i64;
    for i in 0 .. len {
        let val = (data[i] as i8) as i64;
        sum = sum + val;
    }
    sum
}

pub struct AdaptiveFilter {
    bpp: usize,
    stride: usize,
    selected_filter: FilterType,
    data_none: Vec<u8>,
    data_sub: Vec<u8>,
    data_up: Vec<u8>,
    data_average: Vec<u8>,
    data_paeth: Vec<u8>,
}

impl AdaptiveFilter {
    pub fn new(header: Header) -> AdaptiveFilter {
        let stride_in = header.stride();
        let stride_out = stride_in + 1;
        AdaptiveFilter {
            bpp: header.bytes_per_pixel(),
            stride: stride_in,
            selected_filter: FilterType::None,
            data_none: vec![0; stride_out],
            data_sub: vec![0; stride_out],
            data_up: vec![0; stride_out],
            data_average: vec![0; stride_out],
            data_paeth: vec![0; stride_out],
        }
    }

    pub fn filter(&mut self, prev: &[u8], src: &[u8]) -> &[u8] {
        filter_none(self.bpp, prev, src, &mut self.data_none);
        let complexity_none = estimate_complexity(&self.data_none);
        let mut min = complexity_none;

        filter_sub(self.bpp, prev, src, &mut self.data_sub);
        let complexity_sub = estimate_complexity(&self.data_sub);
        min = cmp::min(min, complexity_sub);

        filter_up(self.bpp, prev, src, &mut self.data_up);
        let complexity_up = estimate_complexity(&self.data_up);
        min = cmp::min(min, complexity_up);

        filter_average(self.bpp, prev, src, &mut self.data_average);
        let complexity_average = estimate_complexity(&self.data_average);
        min = cmp::min(min, complexity_average);

        filter_paeth(self.bpp, prev, src, &mut self.data_paeth);
        let complexity_paeth = estimate_complexity(&self.data_paeth);
        min = cmp::min(min, complexity_sub);

        if min == complexity_paeth {
            &self.data_paeth
        } else if min == complexity_average {
            &self.data_average
        } else if min == complexity_up {
            &self.data_up
        } else if min == complexity_sub {
            &self.data_sub
        } else {
            &self.data_none
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AdaptiveFilter;
    use super::super::Header;
    use super::super::ColorType;

    #[test]
    fn it_works() {
        let header = Header::with_color(1024, 768, ColorType::Truecolor);
        let mut filter = AdaptiveFilter::new(header);

        let prev = vec![0u8; header.stride()];
        let row = vec![0u8; header.stride()];
        let filtered_data = filter.filter(&prev, &row);
        assert_eq!(filtered_data.len(), header.stride() + 1);
    }

    #[test]
    fn it_works_16() {
        let header = Header::with_depth(1024, 768, ColorType::Truecolor, 16);
        let mut filter = AdaptiveFilter::new(header);

        let prev = vec![0u8; header.stride()];
        let row = vec![0u8; header.stride()];
        let filtered_data = filter.filter(&prev, &row);
        assert_eq!(filtered_data.len(), header.stride() + 1);
    }
}
