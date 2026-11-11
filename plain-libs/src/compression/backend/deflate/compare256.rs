pub fn compare256_slice(src0: &[u8], src1: &[u8]) -> usize {
    let src0 = first_chunk::<_, 256>(src0).unwrap();
    let src1 = first_chunk::<_, 256>(src1).unwrap();
    compare256(src0, src1)
}
fn compare256(src0: &[u8; 256], src1: &[u8; 256]) -> usize {
    #[cfg(target_arch = "x86_64")]
    if crate::cpu_features::is_enabled_avx2_and_bmi2() {
        return unsafe { avx2::compare256(src0, src1) };
    }
    #[cfg(target_arch = "aarch64")]
    if crate::cpu_features::is_enabled_neon() {
        return unsafe { neon::compare256(src0, src1) };
    }
    #[cfg(target_arch = "wasm32")]
    if crate::cpu_features::is_enabled_simd128() {
        return wasm32::compare256(src0, src1);
    }
    rust::compare256(src0, src1)
}
pub fn compare256_rle_slice(byte: u8, src: &[u8]) -> usize {
    rust::compare256_rle(byte, src)
}
#[inline]
pub const fn first_chunk<T, const N: usize>(slice: &[T]) -> Option<&[T; N]> {
    if slice.len() < N {
        None
    } else {
        Some(unsafe { &*(slice.as_ptr() as *const [T; N]) })
    }
}
mod rust {
    pub fn compare256(src0: &[u8; 256], src1: &[u8; 256]) -> usize {
        src0.iter().zip(src1).take_while(|(x, y)| x == y).count()
    }
    pub fn compare256_rle(byte: u8, src: &[u8]) -> usize {
        assert!(src.len() >= 256, "too short {}", src.len());
        let sv = u64::from_ne_bytes([byte; 8]);
        let mut len = 0;
        for chunk in src[..256].chunks_exact(8) {
            let mv = u64::from_le_bytes(chunk.try_into().unwrap());
            let diff = sv ^ mv;
            if diff > 0 {
                let match_byte = diff.trailing_zeros() / 8;
                return len + match_byte as usize;
            }
            len += 8
        }
        256
    }
}
#[cfg(target_arch = "aarch64")]
mod neon {
    use core::arch::aarch64::{
        uint8x16x4_t, vceqq_u8, vget_lane_u64, vld4q_u8, vreinterpret_u64_u8, vreinterpretq_u16_u8,
        vrev16q_u8, vshrn_n_u16, vsriq_n_u8,
    };
    #[target_feature(enable = "neon")]
    pub unsafe fn compare256(src0: &[u8; 256], src1: &[u8; 256]) -> usize {
        {
            type Chunk = uint8x16x4_t;
            let src0 = src0.chunks_exact(core::mem::size_of::<Chunk>());
            let src1 = src1.chunks_exact(core::mem::size_of::<Chunk>());
            let mut len = 0;
            for (a, b) in src0.zip(src1) {
                unsafe {
                    let a: Chunk = vld4q_u8(a.as_ptr());
                    let b: Chunk = vld4q_u8(b.as_ptr());
                    let cmp0 = vceqq_u8(a.0, b.0);
                    let cmp1 = vceqq_u8(a.1, b.1);
                    let cmp2 = vceqq_u8(a.2, b.2);
                    let cmp3 = vceqq_u8(a.3, b.3);
                    let first_two_bits = vsriq_n_u8::<1>(cmp1, cmp0);
                    let last_two_bits = vsriq_n_u8::<1>(cmp3, cmp2);
                    let first_four_bits = vsriq_n_u8::<2>(last_two_bits, first_two_bits);
                    let bitmask_vector = vsriq_n_u8::<4>(first_four_bits, first_four_bits);
                    let bitmask_vector = if cfg!(target_endian = "big") {
                        vrev16q_u8(bitmask_vector)
                    } else {
                        bitmask_vector
                    };
                    let result_vector = vshrn_n_u16::<4>(vreinterpretq_u16_u8(bitmask_vector));
                    let bitmask = vget_lane_u64::<0>(vreinterpret_u64_u8(result_vector));
                    let bitmask = bitmask.to_le();
                    if bitmask != u64::MAX {
                        let match_byte = bitmask.trailing_ones();
                        return len + match_byte as usize;
                    }
                    len += core::mem::size_of::<Chunk>();
                }
            }
            256
        }
    }
}
#[cfg(target_arch = "x86_64")]
mod avx2 {
    use core::arch::x86_64::{
        __m256i, _mm256_cmpeq_epi8, _mm256_loadu_si256, _mm256_movemask_epi8,
    };
    #[target_feature(enable = "avx2")]
    #[target_feature(enable = "bmi2")]
    #[target_feature(enable = "bmi1")]
    pub unsafe fn compare256(src0: &[u8; 256], src1: &[u8; 256]) -> usize {
        unsafe {
            let src0 = src0.chunks_exact(32);
            let src1 = src1.chunks_exact(32);
            let mut len = 0;
            unsafe {
                for (chunk0, chunk1) in src0.zip(src1) {
                    let ymm_src0 = _mm256_loadu_si256(chunk0.as_ptr() as *const __m256i);
                    let ymm_src1 = _mm256_loadu_si256(chunk1.as_ptr() as *const __m256i);
                    let ymm_cmp = _mm256_cmpeq_epi8(ymm_src0, ymm_src1);
                    let mask = _mm256_movemask_epi8(ymm_cmp) as u32;
                    if mask != 0xFFFFFFFF {
                        let match_byte = mask.trailing_ones();
                        return len + match_byte as usize;
                    }
                    len += 32;
                }
            }
            256
        }
    }
}
#[cfg(target_arch = "wasm32")]
mod wasm32 {
    use core::arch::wasm32::{u8x16_bitmask, u8x16_eq, v128, v128_load};
    #[target_feature(enable = "simd128")]
    pub fn compare256(src0: &[u8; 256], src1: &[u8; 256]) -> usize {
        let src0 = src0.chunks_exact(16);
        let src1 = src1.chunks_exact(16);
        let mut len = 0;
        for (chunk0, chunk1) in src0.zip(src1) {
            let v128_src0 = unsafe { v128_load(chunk0.as_ptr() as *const v128) };
            let v128_src1 = unsafe { v128_load(chunk1.as_ptr() as *const v128) };
            let v128_cmp = u8x16_eq(v128_src0, v128_src1);
            let mask = u8x16_bitmask(v128_cmp);
            if mask != 0xFFFF {
                let match_byte = mask.trailing_ones();
                return len + match_byte as usize;
            }
            len += 16;
        }
        256
    }
}
