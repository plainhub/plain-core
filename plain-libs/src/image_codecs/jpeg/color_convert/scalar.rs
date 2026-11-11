use core::convert::TryInto;
pub(crate) const Y_CF: i16 = 16384;
pub(crate) const CR_CF: i16 = 22970;
pub(crate) const CB_CF: i16 = 29032;
pub(crate) const C_G_CR_COEF_1: i16 = -11700;
pub(crate) const C_G_CB_COEF_2: i16 = -5638;
pub(crate) const YUV_PREC: i16 = 14;
pub(crate) const YUV_RND: i16 = (1 << (YUV_PREC - 1)) - 1;
#[inline]
fn clamp(a: i32) -> u8 {
    a.clamp(0, 255) as u8
}
pub fn ycbcr_to_rgba_inner_16_scalar<const BGRA: bool>(
    y: &[i16; 16],
    cb: &[i16; 16],
    cr: &[i16; 16],
    output: &mut [u8],
    pos: &mut usize,
) {
    let (_, output_position) = output.split_at_mut(*pos);
    let opt: &mut [u8; 64] = output_position
        .get_mut(0..64)
        .expect("Slice to small cannot write")
        .try_into()
        .unwrap();
    for ((&y, (cb, cr)), out) in y
        .iter()
        .zip(cb.iter().zip(cr.iter()))
        .zip(opt.chunks_exact_mut(4))
    {
        let cr = cr - 128;
        let cb = cb - 128;
        let y0 = i32::from(y) * i32::from(Y_CF) + i32::from(YUV_RND);
        let r = (y0 + i32::from(cr) * i32::from(CR_CF)) >> YUV_PREC;
        let g = (y0
            + i32::from(cr) * i32::from(C_G_CR_COEF_1)
            + i32::from(cb) * i32::from(C_G_CB_COEF_2))
            >> YUV_PREC;
        let b = (y0 + i32::from(cb) * i32::from(CB_CF)) >> YUV_PREC;
        if BGRA {
            out[0] = clamp(b);
            out[1] = clamp(g);
            out[2] = clamp(r);
            out[3] = 255;
        } else {
            out[0] = clamp(r);
            out[1] = clamp(g);
            out[2] = clamp(b);
            out[3] = 255;
        }
    }
    *pos += 64;
}
pub fn ycbcr_to_rgb_inner_16_scalar<const BGRA: bool>(
    y: &[i16; 16],
    cb: &[i16; 16],
    cr: &[i16; 16],
    output: &mut [u8],
    pos: &mut usize,
) {
    let (_, output_position) = output.split_at_mut(*pos);
    let opt: &mut [u8; 48] = output_position
        .get_mut(0..48)
        .expect("Slice to small cannot write")
        .try_into()
        .unwrap();
    for ((&y, (cb, cr)), out) in y
        .iter()
        .zip(cb.iter().zip(cr.iter()))
        .zip(opt.chunks_exact_mut(3))
    {
        let cr = cr - 128;
        let cb = cb - 128;
        let y0 = i32::from(y) * i32::from(Y_CF) + i32::from(YUV_RND);
        let r = (y0 + i32::from(cr) * i32::from(CR_CF)) >> YUV_PREC;
        let g = (y0
            + i32::from(cr) * i32::from(C_G_CR_COEF_1)
            + i32::from(cb) * i32::from(C_G_CB_COEF_2))
            >> YUV_PREC;
        let b = (y0 + i32::from(cb) * i32::from(CB_CF)) >> YUV_PREC;
        if BGRA {
            out[0] = clamp(b);
            out[1] = clamp(g);
            out[2] = clamp(r);
        } else {
            out[0] = clamp(r);
            out[1] = clamp(g);
            out[2] = clamp(b);
        }
    }
    *pos += 48;
}
pub fn ycbcr_to_grayscale(y: &[i16], width: usize, padded_width: usize, output: &mut [u8]) {
    for (y_in, out) in y
        .chunks_exact(padded_width)
        .zip(output.chunks_exact_mut(width))
    {
        for (y, out) in y_in.iter().zip(out.iter_mut()) {
            *out = *y as u8;
        }
    }
}
