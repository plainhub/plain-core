use super::to_u64;

#[test]
fn kernel_counters_are_checked_without_arithmetic_panics() {
    assert_eq!(to_u64(b"123"), 123);
    assert_eq!(to_u64(b"18446744073709551615"), u64::MAX);
    for value in [b"".as_slice(), b"-1", b"x", b"18446744073709551616", &[255]] {
        assert_eq!(to_u64(value), 0);
    }
}
