use super::*;

#[test]
fn encodes_a_landscape_and_transparent_portrait_in_the_standard_format() {
    for (width, height, alpha, expected) in [
        (9, 6, false, "UUgKFZJwd3eCh4eHiHiIh4AZCMd3"),
        (6, 9, true, "U9iFCwgYgaCXd4h7ARn4aHN3cIiHeHg="),
    ] {
        let stride = if alpha { 4 } else { 3 };
        let data = (0..width * height)
            .flat_map(|i| {
                let x = i % width;
                let y = i / width;
                [
                    x * 21 + y * 3 + x * y % 5,
                    x * 2 + y * 19 + x * y % 3,
                    30 + x * y * 3 + x * y % 7,
                    80 + x * 6 + y * 8 + x * y % 11,
                ]
                .into_iter()
                .take(stride)
                .map(|v| v as u8)
            })
            .collect();
        let bitmap = if alpha {
            Bitmap::new_rgba(data, width, height)
        } else {
            Bitmap::new_rgb(data, width, height)
        };
        assert_eq!(
            crate::utils::base64::base64_encode(&from_bitmap(&bitmap)),
            expected
        );
    }
}
