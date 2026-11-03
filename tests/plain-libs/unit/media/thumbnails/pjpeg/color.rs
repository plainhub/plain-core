use super::*;

#[test]
fn six_bit_color_rounding_matches_the_vector_contract_on_every_chroma_pair() {
    for cb in 0..=255u8 {
        for cr in 0..=255u8 {
            for y in [0, 1, 127, 128, 254, 255] {
                let blue = f64::from(cb) - 128.0;
                let red = f64::from(cr) - 128.0;
                let rounded = |v: f64| (v + 0.5).floor();
                let luma = f64::from(y) * 64.0 + 32.0;
                let expected = [
                    luma + red * 64.0 + rounded(red * 13173.0 / 512.0),
                    luma - rounded(blue * 11276.0 / 512.0) - rounded(red * 23401.0 / 512.0),
                    luma + blue * 64.0 + rounded(blue * 25297.0 / 512.0),
                ]
                .map(|v| (v / 64.0).floor().clamp(0.0, 255.0) as u8);
                assert_eq!(ycbcr_to_rgb_six_bit(y, cb, cr), expected, "{y},{cb},{cr}");
            }
        }
    }
}
