use std::{
    mem::{ManuallyDrop, align_of, size_of, size_of_val},
    slice,
};
mod sealed {
    pub trait Sealed {}
    macro_rules! primitives {($($ty:ty),*) => {$(impl Sealed for $ty {})*};}
    primitives!(
        u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, f32, f64
    );
}
pub(crate) trait Pod: sealed::Sealed + Copy + 'static {}
impl<T: sealed::Sealed + Copy + 'static> Pod for T {}
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CastError {
    Alignment,
    Length,
}
#[inline]
pub(crate) fn cast_slice<A: Pod, B: Pod>(input: &[A]) -> &[B] {
    assert_eq!(
        input.as_ptr().align_offset(align_of::<B>()),
        0,
        "unaligned byte view"
    );
    let bytes = size_of_val(input);
    assert_eq!(bytes % size_of::<B>(), 0, "incomplete output element");
    // Sealed primitives accept every bit pattern; alignment and output length were checked.
    unsafe { slice::from_raw_parts(input.as_ptr().cast(), bytes / size_of::<B>()) }
}
#[inline]
pub(crate) fn cast_slice_mut<A: Pod, B: Pod>(input: &mut [A]) -> &mut [B] {
    assert_eq!(
        input.as_ptr().align_offset(align_of::<B>()),
        0,
        "unaligned byte view"
    );
    let bytes = size_of_val(input);
    assert_eq!(bytes % size_of::<B>(), 0, "incomplete output element");
    // Both sealed primitive types accept mutations of every bit pattern through the exclusive borrow.
    unsafe { slice::from_raw_parts_mut(input.as_mut_ptr().cast(), bytes / size_of::<B>()) }
}
pub(crate) fn try_cast_vec<A: Pod, B: Pod>(input: Vec<A>) -> Result<Vec<B>, (CastError, Vec<A>)> {
    if align_of::<A>() != align_of::<B>() {
        return Err((CastError::Alignment, input));
    }
    let bytes = input.len() * size_of::<A>();
    let capacity = input.capacity() * size_of::<A>();
    if bytes % size_of::<B>() != 0 || capacity % size_of::<B>() != 0 {
        return Err((CastError::Length, input));
    }
    let mut input = ManuallyDrop::new(input);
    // The allocation layout is identical and sealed primitives have no invalid bit patterns or destructors.
    Ok(unsafe {
        Vec::from_raw_parts(
            input.as_mut_ptr().cast(),
            bytes / size_of::<B>(),
            capacity / size_of::<B>(),
        )
    })
}
#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/byte_cast.rs"]
mod tests;
