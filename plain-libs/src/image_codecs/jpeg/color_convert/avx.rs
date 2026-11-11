#![cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use crate::image_codecs::jpeg::color_convert::scalar::{
    C_G_CB_COEF_2, C_G_CR_COEF_1, CB_CF, CR_CF, Y_CF, YUV_RND,
};
#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;
pub union YmmRegister {
    mm256: __m256i,
    array: [i16; 16],
}
const R_AVX_COEF: i32 = i32::from_ne_bytes([CR_CF.to_ne_bytes()[0], CR_CF.to_ne_bytes()[1], 0, 0]);
const B_AVX_COEF: i32 = i32::from_ne_bytes([0, 0, CB_CF.to_ne_bytes()[0], CB_CF.to_ne_bytes()[1]]);
const G_COEF_AVX_COEF: i32 = i32::from_ne_bytes([
    C_G_CR_COEF_1.to_ne_bytes()[0],
    C_G_CR_COEF_1.to_ne_bytes()[1],
    C_G_CB_COEF_2.to_ne_bytes()[0],
    C_G_CB_COEF_2.to_ne_bytes()[1],
]);
#[inline(always)]
pub fn ycbcr_to_rgb_avx2(
    y: &[i16; 16],
    cb: &[i16; 16],
    cr: &[i16; 16],
    out: &mut [u8],
    offset: &mut usize,
) {
    unsafe {
        ycbcr_to_rgb_avx2_1(y, cb, cr, out, offset);
    }
}
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn ycbcr_to_rgb_avx2_1(
    y: &[i16; 16],
    cb: &[i16; 16],
    cr: &[i16; 16],
    out: &mut [u8],
    offset: &mut usize,
) {
    unsafe {
        let (mut r, mut g, mut b) = ycbcr_to_rgb_baseline_no_clamp(y, cb, cr);
        r = _mm256_packus_epi16(r, _mm256_setzero_si256());
        g = _mm256_packus_epi16(g, _mm256_setzero_si256());
        b = _mm256_packus_epi16(b, _mm256_setzero_si256());
        r = _mm256_permute4x64_epi64::<{ shuffle(3, 1, 2, 0) }>(r);
        g = _mm256_permute4x64_epi64::<{ shuffle(3, 1, 2, 0) }>(g);
        b = _mm256_permute4x64_epi64::<{ shuffle(3, 1, 2, 0) }>(b);
        let sh_r = _mm256_setr_epi8(
            0, 11, 6, 1, 12, 7, 2, 13, 8, 3, 14, 9, 4, 15, 10, 5, 0, 11, 6, 1, 12, 7, 2, 13, 8, 3,
            14, 9, 4, 15, 10, 5,
        );
        let sh_g = _mm256_setr_epi8(
            5, 0, 11, 6, 1, 12, 7, 2, 13, 8, 3, 14, 9, 4, 15, 10, 5, 0, 11, 6, 1, 12, 7, 2, 13, 8,
            3, 14, 9, 4, 15, 10,
        );
        let sh_b = _mm256_setr_epi8(
            10, 5, 0, 11, 6, 1, 12, 7, 2, 13, 8, 3, 14, 9, 4, 15, 10, 5, 0, 11, 6, 1, 12, 7, 2, 13,
            8, 3, 14, 9, 4, 15,
        );
        let r0 = _mm256_shuffle_epi8(r, sh_r);
        let g0 = _mm256_shuffle_epi8(g, sh_g);
        let b0 = _mm256_shuffle_epi8(b, sh_b);
        let m0 = _mm256_setr_epi8(
            0, -1, 0, 0, -1, 0, 0, -1, 0, 0, -1, 0, 0, -1, 0, 0, 0, -1, 0, 0, -1, 0, 0, -1, 0, 0,
            -1, 0, 0, -1, 0, 0,
        );
        let m1 = _mm256_setr_epi8(
            0, 0, -1, 0, 0, -1, 0, 0, -1, 0, 0, -1, 0, 0, -1, 0, 0, 0, -1, 0, 0, -1, 0, 0, -1, 0,
            0, -1, 0, 0, -1, 0,
        );
        let p0 = _mm256_blendv_epi8(_mm256_blendv_epi8(r0, g0, m0), b0, m1);
        let p1 = _mm256_blendv_epi8(_mm256_blendv_epi8(g0, b0, m0), r0, m1);
        let p2 = _mm256_blendv_epi8(_mm256_blendv_epi8(b0, r0, m0), g0, m1);
        let rgb0 = _mm256_permute2x128_si256::<32>(p0, p1);
        let rgb1 = _mm256_permute2x128_si256::<48>(p2, p0);
        _mm256_storeu_si256(out.as_mut_ptr().cast(), rgb0);
        _mm_storeu_si128(out[32..].as_mut_ptr().cast(), _mm256_castsi256_si128(rgb1));
        *offset += 48;
    }
}
#[inline]
#[target_feature(enable = "avx2")]
unsafe fn ycbcr_to_rgb_baseline_no_clamp(
    y: &[i16; 16],
    cb: &[i16; 16],
    cr: &[i16; 16],
) -> (__m256i, __m256i, __m256i) {
    unsafe {
        let y_c = _mm256_loadu_si256(y.as_ptr().cast());
        let cb_c = _mm256_loadu_si256(cb.as_ptr().cast());
        let cr_c = _mm256_loadu_si256(cr.as_ptr().cast());
        let y_coeff = _mm256_set1_epi32(i32::from(Y_CF));
        let cr_coeff = _mm256_set1_epi32(R_AVX_COEF);
        let cb_coeff = _mm256_set1_epi32(B_AVX_COEF);
        let cg_coeff = _mm256_set1_epi32(G_COEF_AVX_COEF);
        let v_rnd = _mm256_set1_epi32(i32::from(YUV_RND));
        let uv_bias = _mm256_set1_epi16(128);
        let v_0 = _mm256_slli_epi16::<8>(cb_c);
        let u_v_8 = _mm256_or_si256(v_0, cr_c);
        let mut u_v_lo = _mm256_unpacklo_epi8(u_v_8, _mm256_setzero_si256());
        let mut u_v_hi = _mm256_unpackhi_epi8(u_v_8, _mm256_setzero_si256());
        let mut y_lo = _mm256_unpacklo_epi16(y_c, _mm256_setzero_si256());
        let mut y_hi = _mm256_unpackhi_epi16(y_c, _mm256_setzero_si256());
        u_v_lo = _mm256_sub_epi16(u_v_lo, uv_bias);
        u_v_hi = _mm256_sub_epi16(u_v_hi, uv_bias);
        y_lo = _mm256_madd_epi16(y_lo, y_coeff);
        y_hi = _mm256_madd_epi16(y_hi, y_coeff);
        let mut r_lo = _mm256_madd_epi16(u_v_lo, cr_coeff);
        let mut r_hi = _mm256_madd_epi16(u_v_hi, cr_coeff);
        let mut g_lo = _mm256_madd_epi16(u_v_lo, cg_coeff);
        let mut g_hi = _mm256_madd_epi16(u_v_hi, cg_coeff);
        y_lo = _mm256_add_epi32(y_lo, v_rnd);
        y_hi = _mm256_add_epi32(y_hi, v_rnd);
        let mut b_lo = _mm256_madd_epi16(u_v_lo, cb_coeff);
        let mut b_hi = _mm256_madd_epi16(u_v_hi, cb_coeff);
        r_lo = _mm256_add_epi32(r_lo, y_lo);
        r_hi = _mm256_add_epi32(r_hi, y_hi);
        g_lo = _mm256_add_epi32(g_lo, y_lo);
        g_hi = _mm256_add_epi32(g_hi, y_hi);
        b_lo = _mm256_add_epi32(b_lo, y_lo);
        b_hi = _mm256_add_epi32(b_hi, y_hi);
        r_lo = _mm256_srai_epi32::<14>(r_lo);
        r_hi = _mm256_srai_epi32::<14>(r_hi);
        g_lo = _mm256_srai_epi32::<14>(g_lo);
        g_hi = _mm256_srai_epi32::<14>(g_hi);
        b_lo = _mm256_srai_epi32::<14>(b_lo);
        b_hi = _mm256_srai_epi32::<14>(b_hi);
        let r = _mm256_packus_epi32(r_lo, r_hi);
        let g = _mm256_packus_epi32(g_lo, g_hi);
        let b = _mm256_packus_epi32(b_lo, b_hi);
        return (r, g, b);
    }
}
#[inline(always)]
pub fn ycbcr_to_rgba_avx2(
    y: &[i16; 16],
    cb: &[i16; 16],
    cr: &[i16; 16],
    out: &mut [u8],
    offset: &mut usize,
) {
    unsafe {
        ycbcr_to_rgba_unsafe(y, cb, cr, out, offset);
    }
}
# [inline] # [target_feature (enable = "avx2")] # [rustfmt :: skip] unsafe fn ycbcr_to_rgba_unsafe (y : & [i16 ; 16] , cb : & [i16 ; 16] , cr : & [i16 ; 16] , out : & mut [u8] , offset : & mut usize ,) { unsafe { let tmp : & mut [u8 ; 64] = out . get_mut (* offset .. * offset + 64) . expect ("Slice to small cannot write") . try_into () . unwrap () ; let (r , g , b) = ycbcr_to_rgb_baseline_no_clamp (y , cb , cr) ; let c = _mm256_packus_epi16 (r , g) ; let d = _mm256_packus_epi16 (b , _mm256_set1_epi16 (255)) ; let e = _mm256_unpacklo_epi8 (c , d) ; let f = _mm256_unpackhi_epi8 (c , d) ; let g = _mm256_unpacklo_epi8 (e , f) ; let h = _mm256_unpackhi_epi8 (e , f) ; let i = _mm256_permute2x128_si256 :: < { shuffle (3 , 2 , 1 , 0) } > (g , h) ; let j = _mm256_permute2x128_si256 :: < { shuffle (1 , 2 , 3 , 0) } > (g , h) ; let k = _mm256_permute2x128_si256 :: < { shuffle (3 , 2 , 0 , 1) } > (g , h) ; let l = _mm256_permute2x128_si256 :: < { shuffle (0 , 3 , 2 , 1) } > (g , h) ; let m = _mm256_blend_epi32 :: < 0b1111_0000 > (i , j) ; let n = _mm256_blend_epi32 :: < 0b1111_0000 > (k , l) ; _mm256_storeu_si256 (tmp . as_mut_ptr () . cast () , m) ; _mm256_storeu_si256 (tmp [32 ..] . as_mut_ptr () . cast () , n) ; * offset += 64 ; } }
#[inline]
const fn shuffle(z: i32, y: i32, x: i32, w: i32) -> i32 {
    (z << 6) | (y << 4) | (x << 2) | w
}
