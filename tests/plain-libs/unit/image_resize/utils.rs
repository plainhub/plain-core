use super::simd_slice;
use crate::image_resize::pixels::U8x3;

#[test]
fn simd_windows_measure_remaining_bytes_for_pixels_and_coefficients() {
    let rgb = [U8x3::default(); 6];
    assert_eq!(simd_slice(&rgb, 0, 16).len(), 6);
    assert_eq!(simd_slice(&rgb, 2, 12).len(), 4);
    assert_eq!(simd_slice(&[1i16; 4], 0, 8), &[1; 4]);
    assert!(simd_slice::<u8>(&[], 0, 0).is_empty());
}

#[test]
#[should_panic(expected = "SIMD load exceeds row bounds")]
fn simd_windows_reject_a_vector_that_crosses_the_last_rgb_pixel() {
    simd_slice(&[U8x3::default(); 5], 0, 16);
}

#[test]
#[should_panic(expected = "SIMD load exceeds row bounds")]
fn simd_windows_measure_from_the_requested_offset() {
    simd_slice(&[0u8; 32], 17, 16);
}

#[cfg(target_arch = "x86_64")]
#[test]
#[should_panic(expected = "SIMD load exceeds row bounds")]
fn sse_loader_rejects_a_short_rgb_row_before_the_intrinsic_read() {
    unsafe { crate::image_resize::simd_utils::loadu_si128(&[U8x3::default(); 5], 0) };
}
