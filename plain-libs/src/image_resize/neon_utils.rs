use crate::image_resize::pixels::U8x4;
use crate::image_resize::utils::simd_slice;
use std::arch::aarch64::*;

#[inline(always)]
pub unsafe fn load_u8x4(buf: &[U8x4], index: usize) -> uint8x8_t {
    vcreate_u8(u32::from_le_bytes(buf[index].0) as u64)
}

#[inline(always)]
pub unsafe fn load_u8x8<T>(buf: &[T], index: usize) -> uint8x8_t {
    vld1_u8(simd_slice(buf, index, 8).as_ptr() as *const u8)
}

#[inline(always)]
pub unsafe fn load_u8x8x3<T>(buf: &[T], index: usize) -> uint8x8x3_t {
    vld1_u8_x3(simd_slice(buf, index, 24).as_ptr() as *const u8)
}

#[inline(always)]
pub unsafe fn load_deintrel_u8x8x4<T>(buf: &[T], index: usize) -> uint8x8x4_t {
    vld4_u8(simd_slice(buf, index, 32).as_ptr() as *const u8)
}

#[inline(always)]
pub unsafe fn load_u8x16<T>(buf: &[T], index: usize) -> uint8x16_t {
    vld1q_u8(simd_slice(buf, index, 16).as_ptr() as *const u8)
}

#[inline(always)]
pub unsafe fn load_u8x16x2<T>(buf: &[T], index: usize) -> uint8x16x2_t {
    vld1q_u8_x2(simd_slice(buf, index, 32).as_ptr() as *const u8)
}

#[inline(always)]
pub unsafe fn load_u8x16x4<T>(buf: &[T], index: usize) -> uint8x16x4_t {
    vld1q_u8_x4(simd_slice(buf, index, 64).as_ptr() as *const u8)
}

#[inline(always)]
pub unsafe fn load_deintrel_u8x16x4<T>(buf: &[T], index: usize) -> uint8x16x4_t {
    vld4q_u8(simd_slice(buf, index, 64).as_ptr() as *const u8)
}

#[inline(always)]
pub unsafe fn load_i32x4(buf: &[i32], index: usize) -> int32x4_t {
    vld1q_s32(buf[index..][..4].as_ptr() as *const i32)
}

#[inline(always)]
pub unsafe fn store_i32x4(buf: &mut [i32], index: usize, v: int32x4_t) {
    vst1q_s32(buf[index..][..4].as_mut_ptr() as *mut i32, v);
}

#[inline(always)]
pub unsafe fn load_i32x4x2(buf: &[i32], index: usize) -> int32x4x2_t {
    vld1q_s32_x2(buf[index..][..8].as_ptr() as *const i32)
}

#[inline(always)]
pub unsafe fn store_i32x4x2(buf: &mut [i32], index: usize, v: int32x4x2_t) {
    vst1q_s32_x2(buf[index..][..8].as_mut_ptr() as *mut i32, v);
}

#[inline(always)]
pub unsafe fn load_i32x4x4(buf: &[i32], index: usize) -> int32x4x4_t {
    vld1q_s32_x4(buf[index..][..16].as_ptr() as *const i32)
}

#[inline(always)]
pub unsafe fn store_i32x4x4(buf: &mut [i32], index: usize, v: int32x4x4_t) {
    vst1q_s32_x4(buf[index..][..16].as_mut_ptr() as *mut i32, v);
}

#[inline(always)]
pub unsafe fn load_i16x4(buf: &[i16], index: usize) -> int16x4_t {
    vld1_s16(buf[index..][..4].as_ptr() as *const i16)
}

#[inline(always)]
pub unsafe fn load_i16x8(buf: &[i16], index: usize) -> int16x8_t {
    vld1q_s16(buf[index..][..8].as_ptr() as *const i16)
}

#[inline(always)]
pub unsafe fn create_u8x16_from_one_u32(buf: &[u8], index: usize) -> uint8x16_t {
    let value = u32::from_le_bytes(buf[index..][..4].try_into().unwrap());
    vreinterpretq_u8_u32(vsetq_lane_u32::<0>(value, vdupq_n_u32(0u32)))
}

#[inline]
#[target_feature(enable = "neon")]
pub unsafe fn mul_color_to_alpha_u8x16(
    color: uint8x16_t,
    alpha_u16: uint16x8x2_t,
    zero: uint8x16_t,
) -> uint8x16_t {
    let color_u16_lo = vreinterpretq_u16_u8(vzip1q_u8(color, zero));
    let mut tmp_res = vmulq_u16(color_u16_lo, alpha_u16.0);
    tmp_res = vaddq_u16(tmp_res, vrshrq_n_u16::<8>(tmp_res));
    let res_u16_lo = vrshrq_n_u16::<8>(tmp_res);

    let color_u16_hi = vreinterpretq_u16_u8(vzip2q_u8(color, zero));
    let mut tmp_res = vmulq_u16(color_u16_hi, alpha_u16.1);
    tmp_res = vaddq_u16(tmp_res, vrshrq_n_u16::<8>(tmp_res));
    let res_u16_hi = vrshrq_n_u16::<8>(tmp_res);

    vcombine_u8(vqmovn_u16(res_u16_lo), vqmovn_u16(res_u16_hi))
}

#[inline]
#[target_feature(enable = "neon")]
pub unsafe fn mul_color_to_alpha_u8x8(
    color: uint8x8_t,
    alpha_u16: uint16x8_t,
    zero: uint8x8_t,
) -> uint8x8_t {
    let color_u16_lo = vreinterpret_u16_u8(vzip1_u8(color, zero));
    let color_u16_hi = vreinterpret_u16_u8(vzip2_u8(color, zero));
    let color_u16 = vcombine_u16(color_u16_lo, color_u16_hi);
    let mut tmp_res = vmulq_u16(color_u16, alpha_u16);
    tmp_res = vaddq_u16(tmp_res, vrshrq_n_u16::<8>(tmp_res));
    let res_u16 = vrshrq_n_u16::<8>(tmp_res);
    vqmovn_u16(res_u16)
}

unsafe fn mul_color_alpha_u16x8(color: uint16x8_t, alpha: uint16x8_t) -> uint16x8_t {
    let res_color_lo_u16 = vrshrn_n_u32::<16>(vmull_u16(vget_low_u16(color), vget_low_u16(alpha)));
    let res_color_hi_u16 = vrshrn_n_u32::<16>(vmull_high_u16(color, alpha));
    vcombine_u16(res_color_lo_u16, res_color_hi_u16)
}

#[inline(always)]
pub unsafe fn mul_color_recip_alpha_u8x16(
    color: uint8x16_t,
    recip_alpha: uint16x8x2_t,
    zero: uint8x16_t,
) -> uint8x16_t {
    let color_u16_lo = vreinterpretq_u16_u8(vzip1q_u8(zero, color));
    let color_u16_hi = vreinterpretq_u16_u8(vzip2q_u8(zero, color));

    let res_u16_lo = mul_color_alpha_u16x8(color_u16_lo, recip_alpha.0);
    let res_u16_hi = mul_color_alpha_u16x8(color_u16_hi, recip_alpha.1);

    vcombine_u8(vqmovn_u16(res_u16_lo), vqmovn_u16(res_u16_hi))
}

#[inline(always)]
pub unsafe fn mul_color_recip_alpha_u8x8(
    color: uint8x8_t,
    recip_alpha: uint16x8_t,
    zero: uint8x8_t,
) -> uint8x8_t {
    let color_u16_lo = vreinterpret_u16_u8(vzip1_u8(zero, color));
    let color_u16_hi = vreinterpret_u16_u8(vzip2_u8(zero, color));
    let color_u16 = vcombine_u16(color_u16_lo, color_u16_hi);

    let res_color_u16 = mul_color_alpha_u16x8(color_u16, recip_alpha);
    vqmovn_u16(res_color_u16)
}
