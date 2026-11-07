#[inline(always)]
pub(crate) fn foreach_with_pre_reading<D, I>(
    mut iter: impl Iterator<Item = I>,
    mut read_data: impl FnMut(I) -> D,
    mut process_data: impl FnMut(D),
) {
    let mut next_data: D;
    if let Some(src) = iter.next() {
        next_data = read_data(src);
        for src in iter {
            let data = next_data;
            next_data = read_data(src);
            process_data(data);
        }
        process_data(next_data);
    }
}

pub(crate) fn image_size(width: u32, height: u32, pixel_size: usize) -> Option<usize> {
    (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(pixel_size)
}

#[inline(always)]
pub(crate) fn simd_slice<T>(buffer: &[T], index: usize, bytes: usize) -> &[T] {
    let remaining = &buffer[index..];
    assert!(
        std::mem::size_of_val(remaining) >= bytes,
        "SIMD load exceeds row bounds"
    );
    remaining
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/image_resize/utils.rs"]
mod tests;
