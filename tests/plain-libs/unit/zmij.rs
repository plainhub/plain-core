use super::*;
#[test]
fn float_bits_round_trip_including_subnormal_and_signed_zero() {
    let mut buffer = Buffer::new();
    let mut bits = 1_u64;
    for _ in 0..10000 {
        bits = bits
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let value = f64::from_bits(bits);
        if value.is_finite() {
            assert_eq!(buffer.format(value).parse::<f64>().unwrap().to_bits(), bits);
        }
    }
    for value in [0.0, -0.0, f64::MIN_POSITIVE, f64::MAX, f64::from_bits(1)] {
        assert_eq!(
            buffer.format(value).parse::<f64>().unwrap().to_bits(),
            value.to_bits()
        );
    }
}
