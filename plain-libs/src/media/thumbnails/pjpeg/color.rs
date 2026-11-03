pub(super) fn native_six_bit_color() -> bool {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        std::is_x86_feature_detected!("ssse3")
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        false
    }
}
// Preserve jpeg-decoder SSSE3 rounding while the final partial vector keeps scalar rounding.
pub(super) fn ycbcr_to_rgb_six_bit(y: u8, cb: u8, cr: u8) -> [u8; 3] {
    static CONTRIBUTIONS: std::sync::LazyLock<[[i32; 4]; 256]> = std::sync::LazyLock::new(|| {
        std::array::from_fn(|v| {
            let signed = v as i32 - 128;
            [
                signed * 64 + ((signed * 13173 + 256) >> 9),
                (signed * 11276 + 256) >> 9,
                (signed * 23401 + 256) >> 9,
                signed * 64 + ((signed * 25297 + 256) >> 9),
            ]
        })
    });
    let luma = i32::from(y) * 64 + 32;
    let blue = CONTRIBUTIONS[usize::from(cb)];
    let red = CONTRIBUTIONS[usize::from(cr)];
    [luma + red[0], luma - blue[1] - red[2], luma + blue[3]].map(|v| (v >> 6).clamp(0, 255) as u8)
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/media/thumbnails/pjpeg/color.rs"]
mod tests;
