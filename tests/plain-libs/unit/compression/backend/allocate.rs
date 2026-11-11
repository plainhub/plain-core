use super::*;
#[test]
fn backend_allocation_preserves_zero_size_and_simd_alignment_contracts() {
    assert!(RUST.allocate_slice_raw::<u8>(0).is_none());
    let ptr = RUST.allocate_slice_raw::<u16>(17).unwrap();
    assert_eq!(ptr.as_ptr().addr() % usize::from(ALIGN), 0);
    unsafe {
        ptr.as_ptr().write(123);
        assert_eq!(ptr.as_ptr().read(), 123);
        RUST.deallocate(ptr.as_ptr(), 17);
    }
}
#[test]
fn unsupported_element_alignment_is_rejected_before_allocation() {
    #[repr(align(128))]
    struct Overaligned;
    assert!(std::panic::catch_unwind(|| RUST.allocate_slice_raw::<Overaligned>(1)).is_err());
}
