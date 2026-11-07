use crate::audio_tags::util::alloc::fallible_vec_from_element;

#[test]
fn vec_fallible_repeat() {
    let u8_vec_len_20 = fallible_vec_from_element(0u8, 20).unwrap();
    assert_eq!(u8_vec_len_20.len(), 20);
    assert!(u8_vec_len_20.iter().all(|e| *e == 0));

    let u64_vec_len_89 = fallible_vec_from_element(0u64, 89).unwrap();
    assert_eq!(u64_vec_len_89.len(), 89);
    assert!(u64_vec_len_89.iter().all(|e| *e == 0));

    let u8_large_vec = fallible_vec_from_element(0u8, u32::MAX as usize);
    assert!(u8_large_vec.is_err());
}
