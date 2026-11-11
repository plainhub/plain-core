use crate::httparse::iter::Bytes;
use core::arch::aarch64::*;
#[inline]
pub fn match_header_name_vectored(bytes: &mut Bytes) {
    while bytes.as_ref().len() >= 16 {
        unsafe {
            let advance = match_header_name_char_16_neon(bytes.as_ref().as_ptr());
            bytes.advance(advance);
            if advance != 16 {
                return;
            }
        }
    }
    super::swar::match_header_name_vectored(bytes);
}
#[inline]
pub fn match_header_value_vectored(bytes: &mut Bytes) {
    while bytes.as_ref().len() >= 16 {
        unsafe {
            let advance = match_header_value_char_16_neon(bytes.as_ref().as_ptr());
            bytes.advance(advance);
            if advance != 16 {
                return;
            }
        }
    }
    super::swar::match_header_value_vectored(bytes);
}
#[inline]
pub fn match_uri_vectored(bytes: &mut Bytes) {
    while bytes.as_ref().len() >= 16 {
        unsafe {
            let advance = match_url_char_16_neon(bytes.as_ref().as_ptr());
            bytes.advance(advance);
            if advance != 16 {
                return;
            }
        }
    }
    super::swar::match_uri_vectored(bytes);
}
const fn bit_set(x: u8) -> bool {
    matches ! (x , b'0' ..= b'9' | b'a' ..= b'z' | b'A' ..= b'Z' | b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'-' | b'.' | b'^' | b'_' | b'`' | b'|' | b'~')
}
const fn build_bitmap() -> ([u8; 16], [u8; 16]) {
    let mut bitmap_0_7 = [0u8; 16];
    let mut bitmap_8_15 = [0u8; 16];
    let mut i = 0;
    while i < 256 {
        if bit_set(i as u8) {
            let (lo, hi) = (i & 0x0F, i >> 4);
            if i < 128 {
                bitmap_0_7[lo] |= 1 << hi;
            } else {
                bitmap_8_15[lo] |= 1 << hi;
            }
        }
        i += 1;
    }
    (bitmap_0_7, bitmap_8_15)
}
const BITMAPS: ([u8; 16], [u8; 16]) = build_bitmap();
#[inline]
unsafe fn match_header_name_char_16_neon(ptr: *const u8) -> usize {
    unsafe {
        let bitmaps = BITMAPS;
        let (bitmap_0_7, _bitmap_8_15) = bitmaps;
        let bitmap_0_7 = vld1q_u8(bitmap_0_7.as_ptr());
        const BITMASK_LOOKUP_DATA: [u8; 16] =
            [1, 2, 4, 8, 16, 32, 64, 128, 1, 2, 4, 8, 16, 32, 64, 128];
        let bitmask_lookup = vld1q_u8(BITMASK_LOOKUP_DATA.as_ptr());
        let input = vld1q_u8(ptr);
        let indices_0_7 = vandq_u8(input, vdupq_n_u8(0x8F));
        let row_0_7 = vqtbl1q_u8(bitmap_0_7, indices_0_7);
        let bitmask = vqtbl1q_u8(bitmask_lookup, vshrq_n_u8(input, 4));
        let bitsets = row_0_7;
        let tmp = vandq_u8(bitsets, bitmask);
        let result = vceqq_u8(tmp, bitmask);
        offsetz(result) as usize
    }
}
#[inline]
unsafe fn match_url_char_16_neon(ptr: *const u8) -> usize {
    unsafe {
        let input = vld1q_u8(ptr);
        let result = vcleq_u8(vdupq_n_u8(b'!'), input);
        let del = vceqq_u8(input, vdupq_n_u8(0x7F));
        let result = vbicq_u8(result, del);
        offsetz(result) as usize
    }
}
#[inline]
unsafe fn match_header_value_char_16_neon(ptr: *const u8) -> usize {
    unsafe {
        let input = vld1q_u8(ptr);
        let result = vcleq_u8(vdupq_n_u8(b' '), input);
        let tab = vceqq_u8(input, vdupq_n_u8(0x09));
        let result = vorrq_u8(result, tab);
        let del = vceqq_u8(input, vdupq_n_u8(0x7F));
        let result = vbicq_u8(result, del);
        offsetz(result) as usize
    }
}
#[inline]
unsafe fn offsetz(x: uint8x16_t) -> u32 {
    unsafe { offsetnz(vmvnq_u8(x)) }
}
#[inline]
unsafe fn offsetnz(x: uint8x16_t) -> u32 {
    unsafe {
        let x = vreinterpretq_u64_u8(x);
        let low: u64 = vgetq_lane_u64::<0>(x);
        let high: u64 = vgetq_lane_u64::<1>(x);
        #[inline]
        fn clz(x: u64) -> u32 {
            for (i, b) in x.to_ne_bytes().iter().copied().enumerate() {
                if b != 0 {
                    return i as u32;
                }
            }
            8
        }
        if low != 0 {
            clz(low)
        } else if high != 0 {
            return 8 + clz(high);
        } else {
            return 16;
        }
    }
}
