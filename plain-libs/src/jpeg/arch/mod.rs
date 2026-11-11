#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod ssse3;
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
mod wasm;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use std::is_x86_feature_detected;
pub fn get_color_convert_line_ycbcr() -> Option<unsafe fn(&[u8], &[u8], &[u8], &mut [u8]) -> usize>
{
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if is_x86_feature_detected!("ssse3") {
            return Some(ssse3::color_convert_line_ycbcr);
        }
    }
    #[cfg(all(target_feature = "simd128", target_arch = "wasm32"))]
    {
        Some(wasm::color_convert_line_ycbcr)
    }
    #[cfg(not(all(target_feature = "simd128", target_arch = "wasm32")))]
    {
        None
    }
}
pub fn get_dequantize_and_idct_block_8x8()
-> Option<unsafe fn(&[i16; 64], &[u16; 64], usize, &mut [u8])> {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if is_x86_feature_detected!("ssse3") {
            return Some(ssse3::dequantize_and_idct_block_8x8);
        }
    }
    #[cfg(all(target_feature = "simd128", target_arch = "wasm32"))]
    {
        Some(wasm::dequantize_and_idct_block_8x8)
    }
    #[cfg(not(all(target_feature = "simd128", target_arch = "wasm32")))]
    {
        None
    }
}
