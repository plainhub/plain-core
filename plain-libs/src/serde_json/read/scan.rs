#[inline]
pub(super) fn find_escape(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < 16 {
        return None;
    }
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    return Some(unsafe { find_vector::<core::arch::aarch64::uint8x16_t>(bytes) });
    #[cfg(target_arch = "x86_64")]
    return Some(unsafe { find_vector::<core::arch::x86_64::__m128i>(bytes) });
    #[cfg(not(any(
        all(target_arch = "aarch64", target_feature = "neon"),
        target_arch = "x86_64"
    )))]
    None
}

#[cfg(any(
    all(target_arch = "aarch64", target_feature = "neon"),
    target_arch = "x86_64"
))]
#[inline]
unsafe fn find_vector<V: crate::memchr::vector::Vector>(bytes: &[u8]) -> usize {
    use crate::memchr::vector::MoveMask;
    let mut chunks = bytes.chunks_exact(V::BYTES);
    unsafe {
        let quote = V::splat(b'"');
        let backslash = V::splat(b'\\');
        let high = V::splat(0xe0);
        let zero = V::splat(0);
        for (index, chunk) in chunks.by_ref().enumerate() {
            #[cfg(test)]
            tests::record_block();
            let chars = V::load_unaligned(chunk.as_ptr());
            let mask = chars
                .cmpeq(quote)
                .or(chars.cmpeq(backslash))
                .or(chars.and(high).cmpeq(zero))
                .movemask();
            if mask.has_non_zero() {
                return index * V::BYTES + mask.first_offset();
            }
        }
    }
    let tail = chunks.remainder();
    let start = bytes.len() - tail.len();
    tail.iter()
        .position(|&byte| super::is_escape(byte, true))
        .map_or(bytes.len(), |index| start + index)
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/serde_json/read/scan.rs"]
mod tests;
