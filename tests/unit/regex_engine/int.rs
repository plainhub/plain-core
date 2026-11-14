use super::*;

#[test]
fn signed_bits_and_unsigned_widths_preserve_search_indices() {
    for bits in 0..=u8::MAX {
        assert_eq!(i8::from_bits(bits).to_bits(), bits);
        assert_eq!(bits.as_usize(), usize::from(bits));
        assert_eq!(usize::from(bits).as_u8(), bits);
    }
    for bits in [0, 1, i32::MAX as u32, 0x8000_0000, u32::MAX] {
        assert_eq!(i32::from_bits(bits).to_bits(), bits);
        assert_eq!(bits.low_u16(), bits as u16);
        assert_eq!(bits.high_u16(), (bits >> 16) as u16);
    }
    for bits in [0, 1, i64::MAX as u64, 0x8000_0000_0000_0000, u64::MAX] {
        assert_eq!(i64::from_bits(bits).to_bits(), bits);
        assert_eq!(bits.low_u32(), bits as u32);
        assert_eq!(bits.high_u32(), (bits >> 32) as u32);
    }
}

#[cfg(debug_assertions)]
#[test]
fn invalid_search_indices_still_panic_in_debug_builds() {
    assert!(std::panic::catch_unwind(|| (-1_i8).as_usize()).is_err());
    assert!(std::panic::catch_unwind(|| (-1_i32).as_usize()).is_err());
    assert!(std::panic::catch_unwind(|| (-1_i64).as_usize()).is_err());
    assert!(std::panic::catch_unwind(|| 256_usize.as_u8()).is_err());
}
