use super::append_decimal_digit;

#[test]
fn decimal_accumulation_matches_wide_arithmetic_without_wrapping_accepted_values() {
    let boundary = u64::MAX / 10;
    let mut significands = vec![
        0,
        1,
        9,
        10,
        99,
        boundary - 1,
        boundary,
        boundary + 1,
        u64::MAX,
    ];
    let mut sample = 1_u64;
    for _ in 0..4096 {
        sample = sample.wrapping_mul(6364136223846793005).wrapping_add(1);
        significands.push(sample);
    }
    for significand in significands {
        for digit in 0..=9 {
            let wide = u128::from(significand) * 10 + u128::from(digit);
            let expected = u64::try_from(wide).ok();
            assert_eq!(
                append_decimal_digit(significand, digit),
                expected,
                "{significand}, {digit}"
            );
        }
    }
}
