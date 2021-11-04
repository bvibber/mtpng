#[cfg(feature = "zlib")]
pub fn adler32(sum: u32, bytes: &[u8]) -> u32 {
    use std::os::raw::c_uint;

    unsafe { libz_sys::adler32(sum, &bytes[0], bytes.len() as c_uint) as u32 }
}

#[cfg(feature = "zlib")]
pub fn adler32_initial() -> u32 {
    use std::ptr;

    unsafe { libz_sys::adler32(0, ptr::null(), 0) as u32 }
}

#[cfg(feature = "zlib")]
pub fn adler32_combine(sum_a: u32, sum_b: u32, len_b: usize) -> u32 {
    use std::os::raw::c_long;

    unsafe { libz_sys::adler32_combine(sum_a, sum_b, len_b as c_long) as u32 }
}

#[cfg(feature = "miniz")]
pub fn adler32(sum: u32, bytes: &[u8]) -> u32 {
    let mut adler = simd_adler32::Adler32::from_checksum(sum);
    adler.write(bytes);
    adler.finish()
}

#[cfg(feature = "miniz")]
pub fn adler32_initial() -> u32 {
    0
}

#[cfg(feature = "miniz")]
pub fn adler32_combine(sum_a: u32, sum_b: u32, len_b: usize) -> u32 {
    const BASE: u32 = 65521;
  
    /* the derivation of this formula is left as an exercise for the reader */
    let rem = len_b as u32;
    
    let mut sum1 = sum_a & 0xffff;
    let mut sum2 = rem * sum_a;
    sum2 %= BASE;
    
    sum1 += (sum_b & 0xffff) + BASE - 1;
    sum2 += ((sum_a >> 16) & 0xffff) + ((sum_b >> 16) & 0xffff) + BASE - rem;

    if sum1 >= BASE {
        sum1 = sum1.wrapping_sub(BASE);
    }
    if sum1 >= BASE {
        sum1 = sum1.wrapping_sub(BASE);
    }
    if sum2 >= (BASE << 1) {
        sum2 = sum1.wrapping_sub(BASE << 1);
    }
    if sum2 >= BASE {
        sum2 = sum1.wrapping_sub(BASE);
    }
    sum1 | (sum2 << 16)
}