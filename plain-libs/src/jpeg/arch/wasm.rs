#[cfg(target_arch = "wasm32")]
use std::arch::wasm32::*;
#[cfg(target_arch = "wasm32")]
#[target_feature(enable = "simd128")]
fn idct8(data: &mut [v128; 8]) {
    let p2 = data[2];
    let p3 = data[6];
    let p1 = i16x8_q15mulr_sat(i16x8_add_sat(p2, p3), i16x8_splat(17734));
    let t2 = i16x8_sub_sat(
        i16x8_sub_sat(p1, p3),
        i16x8_q15mulr_sat(p3, i16x8_splat(27779)),
    );
    let t3 = i16x8_add_sat(p1, i16x8_q15mulr_sat(p2, i16x8_splat(25079)));
    let p2 = data[0];
    let p3 = data[4];
    let t0 = i16x8_add_sat(p2, p3);
    let t1 = i16x8_sub_sat(p2, p3);
    let x0 = i16x8_add_sat(t0, t3);
    let x3 = i16x8_sub_sat(t0, t3);
    let x1 = i16x8_add_sat(t1, t2);
    let x2 = i16x8_sub_sat(t1, t2);
    let t0 = data[7];
    let t1 = data[5];
    let t2 = data[3];
    let t3 = data[1];
    let p3 = i16x8_add_sat(t0, t2);
    let p4 = i16x8_add_sat(t1, t3);
    let p1 = i16x8_add_sat(t0, t3);
    let p2 = i16x8_add_sat(t1, t2);
    let p5 = i16x8_add_sat(p3, p4);
    let p5 = i16x8_add_sat(p5, i16x8_q15mulr_sat(p5, i16x8_splat(5763)));
    let t0 = i16x8_q15mulr_sat(t0, i16x8_splat(9786));
    let t1 = i16x8_add_sat(
        i16x8_add_sat(t1, t1),
        i16x8_q15mulr_sat(t1, i16x8_splat(1741)),
    );
    let t2 = i16x8_add_sat(
        i16x8_add_sat(t2, i16x8_add_sat(t2, t2)),
        i16x8_q15mulr_sat(t2, i16x8_splat(2383)),
    );
    let t3 = i16x8_add_sat(t3, i16x8_q15mulr_sat(t3, i16x8_splat(16427)));
    let p1 = i16x8_sub_sat(p5, i16x8_q15mulr_sat(p1, i16x8_splat(29490)));
    let p2 = i16x8_sub_sat(
        i16x8_sub_sat(i16x8_sub_sat(p5, p2), p2),
        i16x8_q15mulr_sat(p2, i16x8_splat(18446)),
    );
    let p3 = i16x8_sub_sat(i16x8_q15mulr_sat(p3, i16x8_splat(-31509)), p3);
    let p4 = i16x8_q15mulr_sat(p4, i16x8_splat(-12785));
    let t3 = i16x8_add_sat(i16x8_add_sat(p1, p4), t3);
    let t2 = i16x8_add_sat(i16x8_add_sat(p2, p3), t2);
    let t1 = i16x8_add_sat(i16x8_add_sat(p2, p4), t1);
    let t0 = i16x8_add_sat(i16x8_add_sat(p1, p3), t0);
    data[0] = i16x8_add_sat(x0, t3);
    data[7] = i16x8_sub_sat(x0, t3);
    data[1] = i16x8_add_sat(x1, t2);
    data[6] = i16x8_sub_sat(x1, t2);
    data[2] = i16x8_add_sat(x2, t1);
    data[5] = i16x8_sub_sat(x2, t1);
    data[3] = i16x8_add_sat(x3, t0);
    data[4] = i16x8_sub_sat(x3, t0);
}
#[cfg(target_arch = "wasm32")]
#[target_feature(enable = "simd128")]
fn transpose8(data: &mut [v128; 8]) {
    let d01l = i16x8_shuffle::<0, 8, 1, 9, 2, 10, 3, 11>(data[0], data[1]);
    let d23l = i16x8_shuffle::<0, 8, 1, 9, 2, 10, 3, 11>(data[2], data[3]);
    let d45l = i16x8_shuffle::<0, 8, 1, 9, 2, 10, 3, 11>(data[4], data[5]);
    let d67l = i16x8_shuffle::<0, 8, 1, 9, 2, 10, 3, 11>(data[6], data[7]);
    let d01h = i16x8_shuffle::<4, 12, 5, 13, 6, 14, 7, 15>(data[0], data[1]);
    let d23h = i16x8_shuffle::<4, 12, 5, 13, 6, 14, 7, 15>(data[2], data[3]);
    let d45h = i16x8_shuffle::<4, 12, 5, 13, 6, 14, 7, 15>(data[4], data[5]);
    let d67h = i16x8_shuffle::<4, 12, 5, 13, 6, 14, 7, 15>(data[6], data[7]);
    let d0123ll = i32x4_shuffle::<0, 4, 1, 5>(d01l, d23l);
    let d0123lh = i32x4_shuffle::<2, 6, 3, 7>(d01l, d23l);
    let d4567ll = i32x4_shuffle::<0, 4, 1, 5>(d45l, d67l);
    let d4567lh = i32x4_shuffle::<2, 6, 3, 7>(d45l, d67l);
    let d0123hl = i32x4_shuffle::<0, 4, 1, 5>(d01h, d23h);
    let d0123hh = i32x4_shuffle::<2, 6, 3, 7>(d01h, d23h);
    let d4567hl = i32x4_shuffle::<0, 4, 1, 5>(d45h, d67h);
    let d4567hh = i32x4_shuffle::<2, 6, 3, 7>(d45h, d67h);
    data[0] = i64x2_shuffle::<0, 2>(d0123ll, d4567ll);
    data[1] = i64x2_shuffle::<1, 3>(d0123ll, d4567ll);
    data[2] = i64x2_shuffle::<0, 2>(d0123lh, d4567lh);
    data[3] = i64x2_shuffle::<1, 3>(d0123lh, d4567lh);
    data[4] = i64x2_shuffle::<0, 2>(d0123hl, d4567hl);
    data[5] = i64x2_shuffle::<1, 3>(d0123hl, d4567hl);
    data[6] = i64x2_shuffle::<0, 2>(d0123hh, d4567hh);
    data[7] = i64x2_shuffle::<1, 3>(d0123hh, d4567hh);
}
#[cfg(target_arch = "wasm32")]
#[target_feature(enable = "simd128")]
pub fn dequantize_and_idct_block_8x8(
    coefficients: &[i16; 64],
    quantization_table: &[u16; 64],
    output_linestride: usize,
    output: &mut [u8],
) {
    assert!(
        output.len()
            > output_linestride
                .checked_mul(7)
                .unwrap()
                .checked_add(7)
                .unwrap()
    );
    const SHIFT: u32 = 3;
    let mut data = [i16x8_splat(0); 8];
    unsafe {
        for i in 0..8 {
            data[i] = i16x8_shl(
                i16x8_mul(
                    v128_load(coefficients.as_ptr().wrapping_add(i * 8) as *const _),
                    v128_load(quantization_table.as_ptr().wrapping_add(i * 8) as *const _),
                ),
                SHIFT,
            );
        }
    }
    idct8(&mut data);
    transpose8(&mut data);
    idct8(&mut data);
    transpose8(&mut data);
    for i in 0..8 {
        const OFFSET: i16 = 128 << (SHIFT + 3);
        const ROUNDING_BIAS: i16 = (1 << (SHIFT + 3)) >> 1;
        let data_with_offset = i16x8_add_sat(data[i], i16x8_splat(OFFSET + ROUNDING_BIAS));
        unsafe {
            v128_store64_lane::<0>(
                u8x16_narrow_i16x8(i16x8_shr(data_with_offset, SHIFT + 3), i16x8_splat(0)),
                output.as_mut_ptr().wrapping_add(output_linestride * i) as *mut _,
            );
        }
    }
}
#[cfg(target_arch = "wasm32")]
#[target_feature(enable = "simd128")]
pub fn color_convert_line_ycbcr(
    y_slice: &[u8],
    cb_slice: &[u8],
    cr_slice: &[u8],
    output: &mut [u8],
) -> usize {
    assert!(output.len() % 3 == 0);
    let num = output.len() / 3;
    assert!(num <= y_slice.len());
    assert!(num <= cb_slice.len());
    assert!(num <= cr_slice.len());
    let num_vecs = num / 8;
    for i in 0..num_vecs {
        const SHIFT: u32 = 6;
        let y: v128;
        let cb: v128;
        let cr: v128;
        unsafe {
            y = v128_load64_zero(y_slice.as_ptr().wrapping_add(i * 8) as *const _);
            cb = v128_load64_zero(cb_slice.as_ptr().wrapping_add(i * 8) as *const _);
            cr = v128_load64_zero(cr_slice.as_ptr().wrapping_add(i * 8) as *const _);
        }
        let y = i16x8_shl(i16x8_extend_low_u8x16(y), SHIFT);
        let cb = i16x8_shl(i16x8_extend_low_u8x16(cb), SHIFT);
        let cr = i16x8_shl(i16x8_extend_low_u8x16(cr), SHIFT);
        let c128 = i16x8_splat(128 << SHIFT);
        let y = i16x8_add_sat(y, i16x8_splat((1 << SHIFT) >> 1));
        let cb = i16x8_sub_sat(cb, c128);
        let cr = i16x8_sub_sat(cr, c128);
        let cr_140200 = i16x8_add_sat(i16x8_q15mulr_sat(cr, i16x8_splat(13173)), cr);
        let cb_034414 = i16x8_q15mulr_sat(cb, i16x8_splat(11276));
        let cr_071414 = i16x8_q15mulr_sat(cr, i16x8_splat(23401));
        let cb_177200 = i16x8_add_sat(i16x8_q15mulr_sat(cb, i16x8_splat(25297)), cb);
        let r = i16x8_add_sat(y, cr_140200);
        let g = i16x8_sub_sat(y, i16x8_add_sat(cb_034414, cr_071414));
        let b = i16x8_add_sat(y, cb_177200);
        let zero = u8x16_splat(0);
        let r = u8x16_narrow_i16x8(i16x8_shr(r, SHIFT), zero);
        let g = u8x16_narrow_i16x8(i16x8_shr(g, SHIFT), zero);
        let b = u8x16_narrow_i16x8(i16x8_shr(b, SHIFT), zero);
        let rg_lanes =
            i8x16_shuffle::<0, 16, 1, 17, 2, 18, 3, 19, 4, 20, 5, 21, 6, 22, 7, 23>(r, g);
        let rgb_low =
            i8x16_shuffle::<0, 1, 16, 2, 3, 17, 4, 5, 18, 6, 7, 19, 8, 9, 20, 10>(rg_lanes, b);
        let rgb_hi =
            i8x16_shuffle::<11, 21, 12, 13, 22, 14, 15, 23, 0, 0, 0, 0, 0, 0, 0, 0>(rg_lanes, b);
        unsafe {
            v128_store(output.as_mut_ptr().wrapping_add(24 * i) as *mut _, rgb_low);
            v128_store64_lane::<0>(
                rgb_hi,
                output.as_mut_ptr().wrapping_add(24 * i + 16) as *mut _,
            );
        }
    }
    num_vecs * 8
}
