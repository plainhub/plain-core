use super::*;

#[test]
#[should_panic]
fn align_vec_fails() {
    let mut v = AlignedVec::<u16, Align8<[u8; 8]>>::new();
    // This resize must fail. Otherwise, the code below creates a very small actual allocation, and
    // consequently a slice reference that points to memory outside the buffer.
    v.resize(isize::MAX as usize + 2, 0u16);
    // Note that in Rust, no single allocation can exceed `isize::MAX` _bytes_. Meaning it is
    // impossible to soundly create a slice of `u16` with `isize::MAX` elements. If we got to this
    // point, everything is broken already. The indexing will the probably also wrap and appear to
    // work.
    assert_eq!(v.as_slice()[isize::MAX as usize], 0);
}

#[test]
#[should_panic]
fn under_aligned_storage_fails() {
    let mut v = AlignedVec::<u128, Align1<[u8; 1]>>::new();

    // Would be UB: unaligned write of type u128 to pointer aligned to 1 (or 8 in most allocators,
    // but statically we only ensure an alignment of 1).
    v.resize(1, 0u128);
}
