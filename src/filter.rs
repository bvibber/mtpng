use std::cmp;
use std::io::Write;

use super::Header;
use super::ColorType;

#[repr(u8)]
#[derive(Copy, Clone)]
pub enum FilterType {
    None = 0,
    Sub = 1,
    Up = 2,
    Average = 3,
    Paeth = 4,
}

#[derive(Copy, Clone)]
pub enum FilterMode {
    Adaptive,
    Fixed(FilterType),
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

fn estimate_complexity(data: &[u8]) -> i64 {
    let len = data.len();
    let mut sum = 0i64;
    for i in 0 .. len {
        let val = 128 - i64::abs(data[i] as i64 - 128);
        sum = sum + val;
    }
    i64::abs(sum)
}

//
// Holds a target row that can be filtered
// Can be reused.
//
struct Filterator {
    filter: FilterType,
    bpp: usize,
    stride: usize,
    data: Vec<u8>,
    complexity: i64,
}

impl Filterator {
    fn new(filter: FilterType, bpp: usize, stride: usize) -> Filterator {
        Filterator {
            filter: filter,
            bpp: bpp,
            stride: stride,
            data: vec![0u8; stride + 1],
            complexity: 0,
        }
    }

    fn filter(&mut self, prev: &[u8], src: &[u8]) -> &[u8] {
        match self.filter {
            FilterType::None    => filter_none(self.bpp, prev, src, &mut self.data),
            FilterType::Sub     => filter_sub(self.bpp, prev, src, &mut self.data),
            FilterType::Up      => filter_up(self.bpp, prev, src, &mut self.data),
            FilterType::Average => filter_average(self.bpp, prev, src, &mut self.data),
            FilterType::Paeth   => filter_paeth(self.bpp, prev, src, &mut self.data),
        }
        self.complexity = estimate_complexity(&self.data[1..]);
        &self.data
    }

    fn get_data(&self) -> &[u8] {
        &self.data
    }

    fn get_complexity(&self) -> i64 {
        self.complexity
    }
}

pub struct AdaptiveFilter {
    mode: FilterMode,
    filter_none: Filterator,
    filter_up: Filterator,
    filter_sub: Filterator,
    filter_average: Filterator,
    filter_paeth: Filterator,
}

impl AdaptiveFilter {
    pub fn new(header: Header, mode: FilterMode) -> AdaptiveFilter {
        let stride = header.stride();
        let bpp = header.bytes_per_pixel();
        AdaptiveFilter {
            mode: mode,
            filter_none:    Filterator::new(FilterType::None,    bpp, stride),
            filter_up:      Filterator::new(FilterType::Up,      bpp, stride),
            filter_sub:     Filterator::new(FilterType::Sub,     bpp, stride),
            filter_average: Filterator::new(FilterType::Average, bpp, stride),
            filter_paeth:   Filterator::new(FilterType::Paeth,   bpp, stride),
        }
    }

    fn filter_adaptive(&mut self, prev: &[u8], src: &[u8]) -> &[u8] {

        self.filter_none.filter(prev, src);
        let mut min = self.filter_none.get_complexity();

        self.filter_sub.filter(prev, src);
        min = cmp::min(min, self.filter_sub.get_complexity());

        self.filter_up.filter(prev, src);
        min = cmp::min(min, self.filter_up.get_complexity());

        self.filter_average.filter(prev, src);
        min = cmp::min(min, self.filter_average.get_complexity());

        self.filter_paeth.filter(prev, src);
        min = cmp::min(min, self.filter_paeth.get_complexity());

        if min == self.filter_paeth.get_complexity() {
            self.filter_paeth.get_data()
        } else if min == self.filter_average.get_complexity() {
            self.filter_average.get_data()
        } else if min == self.filter_up.get_complexity() {
            self.filter_up.get_data()
        } else if min == self.filter_sub.get_complexity() {
            self.filter_sub.get_data()
        } else {
            self.filter_none.get_data()
        }
    }

    pub fn filter(&mut self, prev: &[u8], src: &[u8]) -> &[u8] {
        match self.mode {
            FilterMode::Fixed(FilterType::None)    => self.filter_none.filter(prev, src),
            FilterMode::Fixed(FilterType::Sub)     => self.filter_sub.filter(prev, src),
            FilterMode::Fixed(FilterType::Up)      => self.filter_up.filter(prev, src),
            FilterMode::Fixed(FilterType::Average) => self.filter_average.filter(prev, src),
            FilterMode::Fixed(FilterType::Paeth)   => self.filter_paeth.filter(prev, src),
            FilterMode::Adaptive                   => self.filter_adaptive(prev, src),
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
