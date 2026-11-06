use super::*;

macro_rules! const_int_str {
    ($n:expr) => {{
        const X: [u8; static_int_str_len($n)] = static_int_str_array($n);
        unsafe { core::str::from_utf8_unchecked(&X) }
    }};
}

#[test]
fn test_const_int_str() {
    const STR_0: &str = const_int_str!(0);
    const STR_4: &str = const_int_str!(4);
    const STR_42: &str = const_int_str!(42);
    const STR_100: &str = const_int_str!(100);
    const STR_999: &str = const_int_str!(999);
    const STR_1236018655: &str = const_int_str!(1236018655);

    assert_eq!(STR_0, "0");
    assert_eq!(STR_4, "4");
    assert_eq!(STR_42, "42");
    assert_eq!(STR_100, "100");
    assert_eq!(STR_999, "999");
    assert_eq!(STR_1236018655, "1236018655");
}

// static encoding tests are in `encoding.rs`

#[test]
fn const_decimal_encoding_keeps_the_full_u64_range() {
    const MAXIMUM: [u8; 20] = static_int_str_array::<20>(u64::MAX);
    const ZERO: [u8; 1] = static_int_str_array::<1>(0);
    assert_eq!(&MAXIMUM, b"18446744073709551615");
    assert_eq!(&ZERO, b"0");
}
