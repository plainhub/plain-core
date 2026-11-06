use super::*;

#[test]
fn h264_rate_uses_the_shared_pixel_budget_and_limits() {
    assert_eq!(h264_bit_rate(320, 240, 24), 4_000_000);
    assert_eq!(h264_bit_rate(1920, 1080, 30), 9_331_200);
    assert_eq!(h264_bit_rate(u32::MAX, u32::MAX, u32::MAX), 50_000_000);
}
