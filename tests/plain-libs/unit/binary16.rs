use super::*;

#[test]
fn round_trips_every_half_encoding_and_quiets_signaling_nans() {
    for bits in 0..=u16::MAX {
        let expected = if bits & 0x7c00 == 0x7c00 && bits & 0x03ff != 0 {
            bits | 0x0200
        } else {
            bits
        };
        assert_eq!(from_f32(to_f32(bits)), expected, "{bits:04x}");
    }
}

#[test]
fn rounds_ties_to_even_at_normal_subnormal_and_overflow_boundaries() {
    assert_eq!(from_f32(1.0 + 2_f32.powi(-11)), 0x3c00);
    assert_eq!(from_f32(1.0 + 3.0 * 2_f32.powi(-11)), 0x3c02);
    assert_eq!(from_f32(2_f32.powi(-25)), 0);
    assert_eq!(from_f32(3.0 * 2_f32.powi(-25)), 2);
    assert_eq!(from_f32(65519.0), 0x7bff);
    assert_eq!(from_f32(65520.0), 0x7c00);
    assert_eq!(from_f32(-0.0), 0x8000);
}

#[test]
fn double_precision_cast_preserves_bits_beyond_a_float32_tie() {
    let tie = 1.0 + 2_f64.powi(-11);
    assert_eq!(from_f64(tie), 0x3c00);
    assert_eq!(from_f64(f64::from_bits(tie.to_bits() + 1)), 0x3c01);
    assert_eq!(from_f64(f64::from_bits(tie.to_bits() - 1)), 0x3c00);
    assert_eq!(from_f64(2_f64.powi(-25)), 0);
    assert_eq!(from_f64(f64::from_bits(2_f64.powi(-25).to_bits() + 1)), 1);
    for bits in 0..=u16::MAX {
        if !to_f32(bits).is_nan() {
            assert_eq!(from_f64(f64::from(to_f32(bits))), bits);
        }
    }
}
