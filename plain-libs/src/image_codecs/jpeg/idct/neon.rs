#![cfg(target_arch = "aarch64")]
use crate::image_codecs::jpeg::unsafe_utils::{YmmRegister, transpose};
use core::arch::aarch64::*;
const SCALE_BITS: i32 = 512 + 65536 + (128 << 17);
#[inline]
#[target_feature(enable = "neon")]
unsafe fn pack_16(a: int32x4x2_t) -> int16x8_t {
    unsafe { vcombine_s16(vqmovn_s32(a.0), vqmovn_s32(a.1)) }
}
#[inline]
#[target_feature(enable = "neon")]
unsafe fn condense_bottom_16(a: int32x4x2_t, b: int32x4x2_t) -> int16x8x2_t {
    unsafe { int16x8x2_t(pack_16(a), pack_16(b)) }
}
#[target_feature(enable = "neon")]
pub unsafe fn idct_neon(in_vector: &mut [i32; 64], out_vector: &mut [i16], stride: usize) {
    unsafe {
        unsafe {
            let mut pos = 0;
            let mut row0 = YmmRegister::load(in_vector[00..].as_ptr().cast());
            let mut row1 = YmmRegister::load(in_vector[08..].as_ptr().cast());
            let mut row2 = YmmRegister::load(in_vector[16..].as_ptr().cast());
            let mut row3 = YmmRegister::load(in_vector[24..].as_ptr().cast());
            let mut row4 = YmmRegister::load(in_vector[32..].as_ptr().cast());
            let mut row5 = YmmRegister::load(in_vector[40..].as_ptr().cast());
            let mut row6 = YmmRegister::load(in_vector[48..].as_ptr().cast());
            let mut row7 = YmmRegister::load(in_vector[56..].as_ptr().cast());
            let row8 = YmmRegister::load(in_vector[1..].as_ptr().cast());
            let or_tree = (((row1 | row8) | (row2 | row3)) | ((row4 | row5) | (row6 | row7)));
            if or_tree.all_zero() {
                let coeff = ((in_vector[0] + 4 + 1024) >> 3).clamp(0, 255) as i16;
                let idct_value = vdupq_n_s16(coeff);
                macro_rules! store {
                    ($ pos : tt ,$ value : tt) => {
                        let mut tmp = [0; 8];
                        vst1q_s16(
                            out_vector
                                .get_mut($pos..$pos + 8)
                                .unwrap_or(&mut tmp)
                                .as_mut_ptr()
                                .cast(),
                            $value,
                        );
                        $pos += stride;
                    };
                }
                store!(pos, idct_value);
                store!(pos, idct_value);
                store!(pos, idct_value);
                store!(pos, idct_value);
                store!(pos, idct_value);
                store!(pos, idct_value);
                store!(pos, idct_value);
                store!(pos, idct_value);
                return;
            }
            macro_rules! dct_pass {
                ($ SCALE_BITS : tt ,$ scale : tt) => {
                    let p1 = (row2 + row6) * 2217;
                    let mut t2 = p1 + row6 * -7567;
                    let mut t3 = p1 + row2 * 3135;
                    let mut t0 = (row0 + row4).const_shl::<12>();
                    let mut t1 = (row0 - row4).const_shl::<12>();
                    let x0 = t0 + t3 + $SCALE_BITS;
                    let x3 = t0 - t3 + $SCALE_BITS;
                    let x1 = t1 + t2 + $SCALE_BITS;
                    let x2 = t1 - t2 + $SCALE_BITS;
                    let p3 = row7 + row3;
                    let p4 = row5 + row1;
                    let p1 = row7 + row1;
                    let p2 = row5 + row3;
                    let p5 = (p3 + p4) * 4816;
                    t0 = row7 * 1223;
                    t1 = row5 * 8410;
                    t2 = row3 * 12586;
                    t3 = row1 * 6149;
                    let p1 = p5 + p1 * -3685;
                    let p2 = p5 + (p2 * -10497);
                    let p3 = p3 * -8034;
                    let p4 = p4 * -1597;
                    t3 += p1 + p4;
                    t2 += p2 + p3;
                    t1 += p2 + p4;
                    t0 += p1 + p3;
                    row0 = (x0 + t3).const_shra::<$scale>();
                    row1 = (x1 + t2).const_shra::<$scale>();
                    row2 = (x2 + t1).const_shra::<$scale>();
                    row3 = (x3 + t0).const_shra::<$scale>();
                    row4 = (x3 - t0).const_shra::<$scale>();
                    row5 = (x2 - t1).const_shra::<$scale>();
                    row6 = (x1 - t2).const_shra::<$scale>();
                    row7 = (x0 - t3).const_shra::<$scale>();
                };
            }
            dct_pass!(512, 10);
            transpose(
                &mut row0, &mut row1, &mut row2, &mut row3, &mut row4, &mut row5, &mut row6,
                &mut row7,
            );
            dct_pass!(SCALE_BITS, 17);
            transpose(
                &mut row0, &mut row1, &mut row2, &mut row3, &mut row4, &mut row5, &mut row6,
                &mut row7,
            );
            macro_rules! permute_store {
                ($ x : tt ,$ y : tt ,$ index : tt ,$ out : tt) => {
                    let a = condense_bottom_16($x, $y);
                    let mut tmp = [0; 8];
                    let b = clamp256_neon(a);
                    vst1q_s16(
                        ($out)
                            .get_mut($index..$index + 8)
                            .unwrap_or(&mut tmp)
                            .as_mut_ptr()
                            .cast(),
                        b.0,
                    );
                    $index += stride;
                    vst1q_s16(
                        ($out)
                            .get_mut($index..$index + 8)
                            .unwrap_or(&mut tmp)
                            .as_mut_ptr()
                            .cast(),
                        b.1,
                    );
                    $index += stride;
                };
            }
            permute_store!((row0.mm256), (row1.mm256), pos, out_vector);
            permute_store!((row2.mm256), (row3.mm256), pos, out_vector);
            permute_store!((row4.mm256), (row5.mm256), pos, out_vector);
            permute_store!((row6.mm256), (row7.mm256), pos, out_vector);
        }
    }
}
#[inline]
#[target_feature(enable = "neon")]
unsafe fn clamp_neon(reg: int16x8_t) -> int16x8_t {
    unsafe {
        let min_s = vdupq_n_s16(0);
        let max_s = vdupq_n_s16(255);
        let max_v = vmaxq_s16(reg, min_s);
        vminq_s16(max_v, max_s)
    }
}
#[inline]
#[target_feature(enable = "neon")]
unsafe fn clamp256_neon(reg: int16x8x2_t) -> int16x8x2_t {
    unsafe { int16x8x2_t(clamp_neon(reg.0), clamp_neon(reg.1)) }
}
