pub fn h264_bit_rate(width: u32, height: u32, frames_per_second: u32) -> u32 {
    (u128::from(width) * u128::from(height) * u128::from(frames_per_second) * 3 / 20)
        .clamp(4_000_000, 50_000_000) as u32
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/media/encoding.rs"]
mod tests;
