use crate::image_resize::utils::simd_slice;
use std::arch::x86_64::*;

use crate::image_resize::pixels::{U8x3, U8x4};

#[inline(always)]
pub unsafe fn loadu_si128<T>(buf: &[T], index: usize) -> __m128i {
    _mm_loadu_si128(simd_slice(buf, index, 16).as_ptr() as *const __m128i)
}

#[inline(always)]
pub unsafe fn loadu_si256<T>(buf: &[T], index: usize) -> __m256i {
    _mm256_loadu_si256(simd_slice(buf, index, 32).as_ptr() as *const __m256i)
}

#[inline(always)]
pub unsafe fn loadl_epi64<T>(buf: &[T], index: usize) -> __m128i {
    _mm_loadl_epi64(simd_slice(buf, index, 8).as_ptr() as *const __m128i)
}

#[inline(always)]
pub unsafe fn mm_cvtepu8_epi32(buf: &[U8x4], index: usize) -> __m128i {
    let v: i32 = i32::from_ne_bytes(buf[index].0);
    _mm_cvtepu8_epi32(_mm_cvtsi32_si128(v))
}

#[inline(always)]
pub unsafe fn mm_cvtepu8_epi32_u8x3(buf: &[U8x3], index: usize) -> __m128i {
    let pixel = buf[index].0;
    let v: i32 = i32::from_le_bytes([pixel[0], pixel[1], pixel[2], 0]);
    _mm_cvtepu8_epi32(_mm_cvtsi32_si128(v))
}

#[inline(always)]
pub unsafe fn mm_cvtepu8_epi32_from_u8(buf: &[u8], index: usize) -> __m128i {
    let value = i32::from_ne_bytes(buf[index..][..4].try_into().unwrap());
    _mm_cvtepu8_epi32(_mm_cvtsi32_si128(value))
}

#[inline(always)]
pub unsafe fn mm_cvtsi32_si128_from_u8(buf: &[u8], index: usize) -> __m128i {
    let value = i32::from_ne_bytes(buf[index..][..4].try_into().unwrap());
    _mm_cvtsi32_si128(value)
}

#[inline(always)]
pub unsafe fn mm_load_and_clone_i16x2(buf: &[i16]) -> __m128i {
    let value = i32::from_ne_bytes({
        let [a, b] = buf[0].to_ne_bytes();
        let [c, d] = buf[1].to_ne_bytes();
        [a, b, c, d]
    });
    _mm_set1_epi32(value)
}

#[inline(always)]
pub unsafe fn mm256_load_and_clone_i16x2(buf: &[i16]) -> __m256i {
    let value = i32::from_ne_bytes({
        let [a, b] = buf[0].to_ne_bytes();
        let [c, d] = buf[1].to_ne_bytes();
        [a, b, c, d]
    });
    _mm256_set1_epi32(value)
}
