use super::{BASE, NMAX};
const UNROLL_MORE: bool = true;
macro_rules! do1 {
    ($ sum1 : expr , $ sum2 : expr , $ chunk : expr , $ i : expr) => {
        $sum1 += unsafe { *$chunk.get_unchecked($i) } as u32;
        $sum2 += $sum1;
    };
}
macro_rules! do2 {
    ($ sum1 : expr , $ sum2 : expr , $ chunk : expr , $ i : expr) => {
        do1!($sum1, $sum2, $chunk, $i);
        do1!($sum1, $sum2, $chunk, $i + 1);
    };
}
macro_rules! do4 {
    ($ sum1 : expr , $ sum2 : expr , $ chunk : expr , $ i : expr) => {
        do2!($sum1, $sum2, $chunk, $i);
        do2!($sum1, $sum2, $chunk, $i + 2);
    };
}
macro_rules! do8 {
    ($ sum1 : expr , $ sum2 : expr , $ chunk : expr , $ i : expr) => {
        do4!($sum1, $sum2, $chunk, $i);
        do4!($sum1, $sum2, $chunk, $i + 4);
    };
}
macro_rules! do16 {
    ($ sum1 : expr , $ sum2 : expr , $ chunk : expr) => {
        do8!($sum1, $sum2, $chunk, 0);
        do8!($sum1, $sum2, $chunk, 8);
    };
}
pub fn adler32_rust(mut adler: u32, buf: &[u8]) -> u32 {
    let mut sum2 = (adler >> 16) & 0xffff;
    adler &= 0xffff;
    if buf.len() == 1 {
        return adler32_len_1(adler, buf, sum2);
    }
    if buf.is_empty() {
        return adler | (sum2 << 16);
    }
    if buf.len() < 16 {
        return adler32_len_16(adler, buf, sum2);
    }
    let mut it = buf.chunks_exact(NMAX as usize);
    for big_chunk in it.by_ref() {
        const N: usize = if UNROLL_MORE { 16 } else { 8 } as usize;
        let it = big_chunk.chunks_exact(N);
        for chunk in it {
            if N == 16 {
                do16!(adler, sum2, chunk);
            } else {
                do8!(adler, sum2, chunk, 0);
            }
        }
        adler %= BASE;
        sum2 %= BASE;
    }
    adler32_len_64(adler, it.remainder(), sum2)
}
pub(crate) fn adler32_len_1(mut adler: u32, buf: &[u8], mut sum2: u32) -> u32 {
    adler += buf[0] as u32;
    adler %= BASE;
    sum2 += adler;
    sum2 %= BASE;
    adler | (sum2 << 16)
}
pub(crate) fn adler32_len_16(mut adler: u32, buf: &[u8], mut sum2: u32) -> u32 {
    for b in buf {
        adler += (*b) as u32;
        sum2 += adler;
    }
    adler %= BASE;
    sum2 %= BASE;
    adler | (sum2 << 16)
}
pub(crate) fn adler32_len_64(mut adler: u32, buf: &[u8], mut sum2: u32) -> u32 {
    const N: usize = if UNROLL_MORE { 16 } else { 8 };
    let mut it = buf.chunks_exact(N);
    for chunk in it.by_ref() {
        if N == 16 {
            do16!(adler, sum2, chunk);
        } else {
            do8!(adler, sum2, chunk, 0);
        }
    }
    adler32_len_16(adler, it.remainder(), sum2)
}
