use crate::image_resize::utils::simd_slice;
use std::arch::wasm32::*;

use crate::image_resize::pixels::{U8x3, U8x4};

#[inline]
#[target_feature(enable = "simd128")]
pub(crate) unsafe fn load_v128<T>(buf: &[T], index: usize) -> v128 {
    v128_load(simd_slice(buf, index, 16).as_ptr() as *const v128)
}

#[inline]
#[target_feature(enable = "simd128")]
pub(crate) unsafe fn loadl_i64<T>(buf: &[T], index: usize) -> v128 {
    let p = simd_slice(buf, index, 8).as_ptr() as *const i64;
    i64x2(p.read_unaligned(), 0)
}

#[inline]
#[target_feature(enable = "simd128")]
pub(crate) unsafe fn ptr_i16_to_set1_i32(buf: &[i16], index: usize) -> v128 {
    let [a, b] = buf[index].to_le_bytes();
    let [c, d] = buf[index + 1].to_le_bytes();
    i32x4_splat(i32::from_le_bytes([a, b, c, d]))
}

#[inline]
#[target_feature(enable = "simd128")]
pub(crate) unsafe fn i32x4_extend_low_ptr_u8(buf: &[u8], index: usize) -> v128 {
    let bytes: [u8; 4] = buf[index..][..4].try_into().unwrap();
    i32x4(
        bytes[0] as i32,
        bytes[1] as i32,
        bytes[2] as i32,
        bytes[3] as i32,
    )
}

#[inline]
#[target_feature(enable = "simd128")]
pub(crate) unsafe fn i32x4_extend_low_ptr_u8x4(buf: &[U8x4], index: usize) -> v128 {
    let v: u32 = u32::from_le_bytes(buf[index].0);
    u32x4_extend_low_u16x8(i16x8_extend_low_u8x16(u32x4(v, 0, 0, 0)))
}

#[inline]
#[target_feature(enable = "simd128")]
pub(crate) unsafe fn i32x4_extend_low_ptr_u8x3(buf: &[U8x3], index: usize) -> v128 {
    let pixel = buf[index].0;
    i32x4(pixel[0] as i32, pixel[1] as i32, pixel[2] as i32, 0)
}

#[inline]
#[target_feature(enable = "simd128")]
pub(crate) unsafe fn i32x4_v128_from_u8(buf: &[u8], index: usize) -> v128 {
    let value = i32::from_le_bytes(buf[index..][..4].try_into().unwrap());
    i32x4(value, 0, 0, 0)
}

pub(crate) unsafe fn u16x8_mul_add_shr16(a_u16x8: v128, b_u16x8: v128, c: v128) -> v128 {
    let lo_u32x4 = u32x4_extmul_low_u16x8(a_u16x8, b_u16x8);
    let hi_u32x4 = u32x4_extmul_high_u16x8(a_u16x8, b_u16x8);
    let lo_u32x4 = u32x4_add(lo_u32x4, c);
    let hi_u32x4 = u32x4_add(hi_u32x4, c);
    i16x8_shuffle::<1, 3, 5, 7, 9, 11, 13, 15>(lo_u32x4, hi_u32x4)
}
