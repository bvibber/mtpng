use super::ColorType;

#[repr(u8)]
pub enum FilterType {
    None = 0,
    Sub = 1,
    Up = 2,
    Average = 3,
    Paeth = 4,
}

fn filter_none(prev: &[u8], src: &[u8], dest: &mut [u8]) {
    dest.clone_from_slice(src);
}

fn filter_sub8(prev: &[u8], src: &[u8], dest: &mut [u8]) {
    // @todo SIMD this
    let len = dest.len();
    let mut last = 0u8;
    for i in 0 .. len {
        let cur = src[i];
        dest[i] = cur - last;
        last = cur;
    }
}

fn filter_sub16(prev: &[u8], src: &[u8], dest: &mut [u8]) {
    // @todo SIMD this
    let len = dest.len() / 2;
    let mut last_high = 0u8;
    let mut last_low = 0u8;
    for i in 0 .. len {
        let i0 = i * 2;
        let i1 = i0 + 1;
        let cur_high = src[i0];
        let cur_low = src[i1];
        dest[i0] = cur_high - last_high;
        dest[i1] = cur_low - last_low;
        last_high = cur_high;
        last_low = cur_low;
    }
}

pub fn filter(filter: FilterType, depth: u8, prev: &[u8], src: &[u8], dest: &mut [u8]) {
    match (filter, depth) {
        (FilterType::None, _depth) => filter_none(prev, src, dest),
        (FilterType::Sub, 8) => filter_sub8(prev, src, dest),
        (FilterType::Sub, 16) => filter_sub16(prev, src, dest),
        _ => panic!("Not yet supported filter combo"),
    }
}

pub struct AdaptiveFilter {
    color_type: ColorType,
    depth: u8,
    stride: usize,
    selected_filter: FilterType,
    data_none: Vec<u8>,
    data_sub: Vec<u8>,
    data_up: Vec<u8>,
    data_average: Vec<u8>,
    data_paeth: Vec<u8>,
}

impl AdaptiveFilter {
    pub fn new(color_type: ColorType, depth: u8, stride: usize) -> AdaptiveFilter {
        AdaptiveFilter {
            color_type: color_type,
            depth: depth,
            stride: stride,
            selected_filter: FilterType::None,
            data_none: Vec::with_capacity(stride + 1),
            data_sub: Vec::with_capacity(stride + 1),
            data_up: Vec::with_capacity(stride + 1),
            data_average: Vec::with_capacity(stride + 1),
            data_paeth: Vec::with_capacity(stride + 1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FilterType;

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
