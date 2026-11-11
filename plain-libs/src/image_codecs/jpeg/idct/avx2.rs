#![cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use crate::image_codecs::jpeg::unsafe_utils::{YmmRegister, transpose};
#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;
const SCALE_BITS: i32 = 512 + 65536 + (128 << 17);
macro_rules! permute_store {
    ($ x : tt ,$ y : tt ,$ index : tt ,$ out : tt ,$ stride : tt) => {
        let a = _mm256_packs_epi32($x, $y);
        let b = clamp_avx(a);
        let mut tmp = [0; 8];
        let c = _mm256_permute4x64_epi64(b, shuffle(3, 1, 2, 0));
        _mm_storeu_si128(
            ($out)
                .get_mut($index..$index + 8)
                .unwrap_or(&mut tmp)
                .as_mut_ptr()
                .cast(),
            _mm256_extractf128_si256::<0>(c),
        );
        $index += $stride;
        _mm_storeu_si128(
            ($out)
                .get_mut($index..$index + 8)
                .unwrap()
                .as_mut_ptr()
                .cast(),
            _mm256_extractf128_si256::<1>(c),
        );
        $index += $stride;
    };
}
#[target_feature(enable = "avx2")]
pub unsafe fn idct_avx2(in_vector: &mut [i32; 64], out_vector: &mut [i16], stride: usize) {
    unsafe {
        let mut pos = 0;
        let rw0 = _mm256_loadu_si256(in_vector[00..].as_ptr().cast());
        let rw1 = _mm256_loadu_si256(in_vector[08..].as_ptr().cast());
        let rw2 = _mm256_loadu_si256(in_vector[16..].as_ptr().cast());
        let rw3 = _mm256_loadu_si256(in_vector[24..].as_ptr().cast());
        let rw4 = _mm256_loadu_si256(in_vector[32..].as_ptr().cast());
        let rw5 = _mm256_loadu_si256(in_vector[40..].as_ptr().cast());
        let rw6 = _mm256_loadu_si256(in_vector[48..].as_ptr().cast());
        let rw7 = _mm256_loadu_si256(in_vector[56..].as_ptr().cast());
        let rw8 = _mm256_loadu_si256(in_vector[1..].as_ptr().cast());
        let mut bitmap = _mm256_or_si256(rw1, rw2);
        bitmap = _mm256_or_si256(bitmap, rw3);
        bitmap = _mm256_or_si256(bitmap, rw4);
        bitmap = _mm256_or_si256(bitmap, rw5);
        bitmap = _mm256_or_si256(bitmap, rw6);
        bitmap = _mm256_or_si256(bitmap, rw7);
        bitmap = _mm256_or_si256(bitmap, rw8);
        if _mm256_testz_si256(bitmap, bitmap) == 1 {
            let coeff = ((in_vector[0] + 4 + 1024) >> 3).clamp(0, 255) as i16;
            let idct_value = _mm_set1_epi16(coeff);
            macro_rules! store {
                ($ pos : tt ,$ value : tt) => {
                    _mm_storeu_si128(
                        out_vector
                            .get_mut($pos..$pos + 8)
                            .unwrap()
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
        let mut row0 = YmmRegister { mm256: rw0 };
        let mut row1 = YmmRegister { mm256: rw1 };
        let mut row2 = YmmRegister { mm256: rw2 };
        let mut row3 = YmmRegister { mm256: rw3 };
        let mut row4 = YmmRegister { mm256: rw4 };
        let mut row5 = YmmRegister { mm256: rw5 };
        let mut row6 = YmmRegister { mm256: rw6 };
        let mut row7 = YmmRegister { mm256: rw7 };
        macro_rules! dct_pass {
            ($ SCALE_BITS : tt ,$ scale : tt) => {
                let p1 = (row2 + row6) * 2217;
                let mut t2 = p1 + row6 * -7567;
                let mut t3 = p1 + row2 * 3135;
                let mut t0 = YmmRegister {
                    mm256: _mm256_slli_epi32((row0 + row4).mm256, 12),
                };
                let mut t1 = YmmRegister {
                    mm256: _mm256_slli_epi32((row0 - row4).mm256, 12),
                };
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
                row0.mm256 = _mm256_srai_epi32((x0 + t3).mm256, $scale);
                row1.mm256 = _mm256_srai_epi32((x1 + t2).mm256, $scale);
                row2.mm256 = _mm256_srai_epi32((x2 + t1).mm256, $scale);
                row3.mm256 = _mm256_srai_epi32((x3 + t0).mm256, $scale);
                row4.mm256 = _mm256_srai_epi32((x3 - t0).mm256, $scale);
                row5.mm256 = _mm256_srai_epi32((x2 - t1).mm256, $scale);
                row6.mm256 = _mm256_srai_epi32((x1 - t2).mm256, $scale);
                row7.mm256 = _mm256_srai_epi32((x0 - t3).mm256, $scale);
            };
        }
        dct_pass!(512, 10);
        transpose(
            &mut row0, &mut row1, &mut row2, &mut row3, &mut row4, &mut row5, &mut row6, &mut row7,
        );
        dct_pass!(SCALE_BITS, 17);
        transpose(
            &mut row0, &mut row1, &mut row2, &mut row3, &mut row4, &mut row5, &mut row6, &mut row7,
        );
        permute_store!((row0.mm256), (row1.mm256), pos, out_vector, stride);
        permute_store!((row2.mm256), (row3.mm256), pos, out_vector, stride);
        permute_store!((row4.mm256), (row5.mm256), pos, out_vector, stride);
        permute_store!((row6.mm256), (row7.mm256), pos, out_vector, stride);
    }
}
#[target_feature(enable = "avx2")]
pub unsafe fn idct_avx2_4x4(in_vector: &mut [i32; 64], out_vector: &mut [i16], stride: usize) {
    unsafe {
        let rw0 = _mm256_loadu_si256(in_vector[00..].as_ptr().cast());
        let rw1 = _mm256_loadu_si256(in_vector[08..].as_ptr().cast());
        let rw2 = _mm256_loadu_si256(in_vector[16..].as_ptr().cast());
        let rw3 = _mm256_loadu_si256(in_vector[24..].as_ptr().cast());
        let mut row0 = YmmRegister { mm256: rw0 };
        let mut row1 = YmmRegister { mm256: rw1 };
        let mut row2 = YmmRegister { mm256: rw2 };
        let mut row3 = YmmRegister { mm256: rw3 };
        let mut row4 = YmmRegister { mm256: rw0 };
        let mut row5 = YmmRegister { mm256: rw0 };
        let mut row6 = YmmRegister { mm256: rw0 };
        let mut row7 = YmmRegister { mm256: rw0 };
        {
            row0.mm256 = _mm256_slli_epi32(row0.mm256, 12);
            row0 += 512;
            let i2 = row2;
            let p1 = i2 * 2217;
            let p3 = i2 * 5352;
            let x0 = row0 + p3;
            let x1 = row0 + p1;
            let x2 = row0 - p1;
            let x3 = row0 - p3;
            let i4 = row3;
            let i3 = row1;
            let p5 = (i4 + i3) * 4816;
            let p1 = p5 + i3 * -3685;
            let p2 = p5 + i4 * -10497;
            let t3 = p5 + i3 * 867;
            let t2 = p5 + i4 * -5945;
            let t1 = p2 + i3 * -1597;
            let t0 = p1 + i4 * -8034;
            row0.mm256 = _mm256_srai_epi32((x0 + t3).mm256, 10);
            row1.mm256 = _mm256_srai_epi32((x1 + t2).mm256, 10);
            row2.mm256 = _mm256_srai_epi32((x2 + t1).mm256, 10);
            row3.mm256 = _mm256_srai_epi32((x3 + t0).mm256, 10);
            row4.mm256 = _mm256_srai_epi32((x3 - t0).mm256, 10);
            row5.mm256 = _mm256_srai_epi32((x2 - t1).mm256, 10);
            row6.mm256 = _mm256_srai_epi32((x1 - t2).mm256, 10);
            row7.mm256 = _mm256_srai_epi32((x0 - t3).mm256, 10);
        }
        transpose(
            &mut row0, &mut row1, &mut row2, &mut row3, &mut row4, &mut row5, &mut row6, &mut row7,
        );
        {
            let i2 = row2;
            let i0 = row0;
            row0.mm256 = _mm256_slli_epi32(i0.mm256, 12);
            let t0 = row0 + SCALE_BITS;
            let t2 = i2 * 2217;
            let t3 = i2 * 5352;
            let x0 = t0 + t3;
            let x3 = t0 - t3;
            let x1 = t0 + t2;
            let x2 = t0 - t2;
            let i3 = row3;
            let i1 = row1;
            let p5 = (i3 + i1) * 4816;
            let p1 = p5 + i1 * -3685;
            let p2 = p5 + i3 * -10497;
            let t3 = p5 + i1 * 867;
            let t2 = p5 + i3 * -5945;
            let t1 = p2 + i1 * -1597;
            let t0 = p1 + i3 * -8034;
            row0.mm256 = _mm256_srai_epi32((x0 + t3).mm256, 17);
            row1.mm256 = _mm256_srai_epi32((x1 + t2).mm256, 17);
            row2.mm256 = _mm256_srai_epi32((x2 + t1).mm256, 17);
            row3.mm256 = _mm256_srai_epi32((x3 + t0).mm256, 17);
            row4.mm256 = _mm256_srai_epi32((x3 - t0).mm256, 17);
            row5.mm256 = _mm256_srai_epi32((x2 - t1).mm256, 17);
            row6.mm256 = _mm256_srai_epi32((x1 - t2).mm256, 17);
            row7.mm256 = _mm256_srai_epi32((x0 - t3).mm256, 17);
        }
        transpose(
            &mut row0, &mut row1, &mut row2, &mut row3, &mut row4, &mut row5, &mut row6, &mut row7,
        );
        let mut pos = 0;
        permute_store!((row0.mm256), (row1.mm256), pos, out_vector, stride);
        permute_store!((row2.mm256), (row3.mm256), pos, out_vector, stride);
        permute_store!((row4.mm256), (row5.mm256), pos, out_vector, stride);
        permute_store!((row6.mm256), (row7.mm256), pos, out_vector, stride);
    }
}
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn clamp_avx(reg: __m256i) -> __m256i {
    unsafe {
        let min_s = _mm256_set1_epi16(0);
        let max_s = _mm256_set1_epi16(255);
        let max_v = _mm256_max_epi16(reg, min_s);
        let min_v = _mm256_min_epi16(max_v, max_s);
        return min_v;
    }
}
#[inline]
const fn shuffle(z: i32, y: i32, x: i32, w: i32) -> i32 {
    ((z << 6) | (y << 4) | (x << 2) | w)
}
