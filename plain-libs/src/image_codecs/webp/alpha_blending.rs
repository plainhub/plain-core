const fn channel_shift(i: u32) -> u32 {
    i * 8
}
fn blend_channel_nonpremult(
    src: u32,
    src_a: u8,
    dst: u32,
    dst_a: u8,
    scale: u32,
    shift: u32,
) -> u8 {
    let src_channel = ((src >> shift) & 0xff) as u8;
    let dst_channel = ((dst >> shift) & 0xff) as u8;
    let blend_unscaled =
        (u32::from(src_channel) * u32::from(src_a)) + (u32::from(dst_channel) * u32::from(dst_a));
    debug_assert!(u64::from(blend_unscaled) < (1u64 << 32) / u64::from(scale));
    ((blend_unscaled * scale) >> channel_shift(3)) as u8
}
fn blend_pixel_nonpremult(src: u32, dst: u32) -> u32 {
    let src_a = ((src >> channel_shift(3)) & 0xff) as u8;
    if src_a == 0 {
        dst
    } else {
        let dst_a = ((dst >> channel_shift(3)) & 0xff) as u8;
        let dst_factor_a = div_by_255(u32::from(dst_a) * (255 - u32::from(src_a)));
        let blend_a = u32::from(src_a) + dst_factor_a;
        let scale = (1u32 << 24) / blend_a;
        let blend_r =
            blend_channel_nonpremult(src, src_a, dst, dst_factor_a as u8, scale, channel_shift(0));
        let blend_g =
            blend_channel_nonpremult(src, src_a, dst, dst_factor_a as u8, scale, channel_shift(1));
        let blend_b =
            blend_channel_nonpremult(src, src_a, dst, dst_factor_a as u8, scale, channel_shift(2));
        debug_assert!(u32::from(src_a) + dst_factor_a < 256);
        (u32::from(blend_r) << channel_shift(0))
            | (u32::from(blend_g) << channel_shift(1))
            | (u32::from(blend_b) << channel_shift(2))
            | (blend_a << channel_shift(3))
    }
}
pub(crate) fn do_alpha_blending(buffer: [u8; 4], canvas: [u8; 4]) -> [u8; 4] {
    blend_pixel_nonpremult(u32::from_le_bytes(buffer), u32::from_le_bytes(canvas)).to_le_bytes()
}
#[inline]
const fn div_by_255(v: u32) -> u32 {
    (((v + 0x80) >> 8) + v + 0x80) >> 8
}
