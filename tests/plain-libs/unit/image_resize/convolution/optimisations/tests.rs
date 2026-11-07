use super::*;
use crate::image_resize::convolution::Bound;
fn get_coefficients(value: f64) -> Coefficients {
    Coefficients {
        values: vec![value],
        window_size: 1,
        bounds: vec![Bound { start: 0, size: 1 }],
    }
}

#[test]
fn test_minimal_precision() {
    // required for some SIMD optimisations
    assert!(Normalizer16::new(get_coefficients(0.0)).precision() >= 4);
    assert!(Normalizer16::new(get_coefficients(2.0)).precision() >= 4);
}

#[test]
fn clipping_preserves_old_lookup_range_and_handles_extreme_accumulators() {
    for precision in [4, 8, 15] {
        let normalizer = Normalizer16 {
            precision,
            chunks: Vec::new(),
        };
        for value in -512_i32..=511 {
            let accumulator = value << precision;
            let expected = if value < 0 {
                0
            } else if value > 255 {
                255
            } else {
                value as u8
            };
            assert_eq!(normalizer.clip(accumulator), expected);
        }
        assert_eq!(normalizer.clip(i32::MIN), 0);
        assert_eq!(normalizer.clip(i32::MAX), 255);
    }
}
