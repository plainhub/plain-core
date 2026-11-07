use super::*;

#[test]
fn test_recip_alpha_array() {
    for alpha in 0..=255u8 {
        let expected = if alpha == 0 {
            0
        } else {
            let scale = (1 << PRECISION) as f64;
            (255.0 * scale / alpha as f64).round() as u32
        };

        let recip_alpha = RECIP_ALPHA[alpha as usize];
        assert_eq!(expected, recip_alpha, "alpha {}", alpha);
    }
}

#[test]
fn test_div_and_clip() {
    let mut err_sum: i32 = 0;
    for alpha in 0..=255u8 {
        for color in 0..=255u8 {
            let expected_color = if alpha == 0 {
                0
            } else {
                let res = color as f64 / (alpha as f64 / 255.);
                res.round().min(255.) as u8
            };

            let recip_alpha = RECIP_ALPHA[alpha as usize];
            let result_color = div_and_clip(color, recip_alpha);
            let delta = result_color as i32 - expected_color as i32;
            err_sum += delta.abs();
        }
    }
    assert_eq!(err_sum, 2512);
}
