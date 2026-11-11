use super::*;

#[test]
fn primitive_views_share_storage_and_keep_native_bytes() {
    let mut words = [0x1234u16, 0x5678];
    let pointer = words.as_ptr().cast::<u8>();
    let bytes: &[u8] = cast_slice(&words);
    assert_eq!(bytes.as_ptr(), pointer);
    assert_eq!(
        bytes,
        [0x1234u16.to_ne_bytes(), 0x5678u16.to_ne_bytes()].concat()
    );
    let bytes: &mut [u8] = cast_slice_mut(&mut words);
    bytes[..2].copy_from_slice(&0xABCDu16.to_ne_bytes());
    assert_eq!(words, [0xABCD, 0x5678]);
}

#[test]
fn compatible_vector_cast_preserves_allocation_and_capacity() {
    let input = vec![1.0f32.to_bits(), (-2.5f32).to_bits()];
    let pointer = input.as_ptr().cast::<f32>();
    let capacity = input.capacity();
    let output: Vec<f32> = try_cast_vec(input).unwrap();
    assert_eq!(output.as_ptr(), pointer);
    assert_eq!(output.capacity(), capacity);
    assert_eq!(output, [1.0, -2.5]);
}

#[test]
fn incompatible_allocation_alignment_returns_the_original_vector() {
    let input = vec![1u16, 2, 3];
    let pointer = input.as_ptr();
    let capacity = input.capacity();
    let (error, original) = try_cast_vec::<u16, u8>(input).unwrap_err();
    assert_eq!(error, CastError::Alignment);
    assert_eq!(original.as_ptr(), pointer);
    assert_eq!(original.capacity(), capacity);
    assert_eq!(original, [1, 2, 3]);
}

#[test]
fn invalid_alignment_and_incomplete_elements_are_rejected() {
    let words = [0u32; 2];
    let bytes: &[u8] = cast_slice(&words);
    assert!(std::panic::catch_unwind(|| cast_slice::<u8, u16>(&bytes[1..3])).is_err());
    assert!(std::panic::catch_unwind(|| cast_slice::<u8, u16>(&bytes[..3])).is_err());
    let empty: &[u16] = &[];
    assert!(cast_slice::<u16, u8>(empty).is_empty());
}
