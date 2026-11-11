use super::*;
#[test]
fn full_width_integer_formatting_matches_decimal_contract() {
    let mut buffer = Buffer::new();
    for value in [
        i128::MIN,
        i128::MAX,
        i64::MIN as i128,
        -1,
        0,
        1,
        999999999999999999,
    ] {
        assert_eq!(buffer.format(value), value.to_string());
    }
    for value in [0, 1, u64::MAX as u128, u128::MAX] {
        assert_eq!(buffer.format(value), value.to_string());
    }
}
