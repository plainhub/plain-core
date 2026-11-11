pub(crate) trait Vector: Copy + core::fmt::Debug {
    const BYTES: usize;
    const ALIGN: usize;
    type Mask: MoveMask;
    unsafe fn splat(byte: u8) -> Self;
    unsafe fn load_aligned(data: *const u8) -> Self;
    unsafe fn load_unaligned(data: *const u8) -> Self;
    unsafe fn movemask(self) -> Self::Mask;
    unsafe fn cmpeq(self, vector2: Self) -> Self;
    unsafe fn and(self, vector2: Self) -> Self;
    unsafe fn or(self, vector2: Self) -> Self;
    unsafe fn movemask_will_have_non_zero(self) -> bool {
        unsafe { self.movemask().has_non_zero() }
    }
}
pub(crate) trait MoveMask: Copy + core::fmt::Debug {
    fn all_zeros_except_least_significant(n: usize) -> Self;
    fn has_non_zero(self) -> bool;
    fn count_ones(self) -> usize;
    fn and(self, other: Self) -> Self;
    fn or(self, other: Self) -> Self;
    fn clear_least_significant_bit(self) -> Self;
    fn first_offset(self) -> usize;
    fn last_offset(self) -> usize;
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct SensibleMoveMask(u32);
impl SensibleMoveMask {
    #[inline(always)]
    fn get_for_offset(self) -> u32 {
        #[cfg(target_endian = "big")]
        {
            self.0.swap_bytes()
        }
        #[cfg(target_endian = "little")]
        {
            self.0
        }
    }
}
impl MoveMask for SensibleMoveMask {
    #[inline(always)]
    fn all_zeros_except_least_significant(n: usize) -> SensibleMoveMask {
        debug_assert!(n < 32);
        SensibleMoveMask(!((1 << n) - 1))
    }
    #[inline(always)]
    fn has_non_zero(self) -> bool {
        self.0 != 0
    }
    #[inline(always)]
    fn count_ones(self) -> usize {
        self.0.count_ones() as usize
    }
    #[inline(always)]
    fn and(self, other: SensibleMoveMask) -> SensibleMoveMask {
        SensibleMoveMask(self.0 & other.0)
    }
    #[inline(always)]
    fn or(self, other: SensibleMoveMask) -> SensibleMoveMask {
        SensibleMoveMask(self.0 | other.0)
    }
    #[inline(always)]
    fn clear_least_significant_bit(self) -> SensibleMoveMask {
        SensibleMoveMask(self.0 & (self.0 - 1))
    }
    #[inline(always)]
    fn first_offset(self) -> usize {
        self.get_for_offset().trailing_zeros() as usize
    }
    #[inline(always)]
    fn last_offset(self) -> usize {
        32 - self.get_for_offset().leading_zeros() as usize - 1
    }
}
#[cfg(target_arch = "x86_64")]
mod x86sse2 {
    use super::{SensibleMoveMask, Vector};
    use core::arch::x86_64::*;
    impl Vector for __m128i {
        const BYTES: usize = 16;
        const ALIGN: usize = Self::BYTES - 1;
        type Mask = SensibleMoveMask;
        #[inline(always)]
        unsafe fn splat(byte: u8) -> __m128i {
            _mm_set1_epi8(byte as i8)
        }
        #[inline(always)]
        unsafe fn load_aligned(data: *const u8) -> __m128i {
            _mm_load_si128(data as *const __m128i)
        }
        #[inline(always)]
        unsafe fn load_unaligned(data: *const u8) -> __m128i {
            _mm_loadu_si128(data as *const __m128i)
        }
        #[inline(always)]
        unsafe fn movemask(self) -> SensibleMoveMask {
            SensibleMoveMask(_mm_movemask_epi8(self) as u32)
        }
        #[inline(always)]
        unsafe fn cmpeq(self, vector2: Self) -> __m128i {
            _mm_cmpeq_epi8(self, vector2)
        }
        #[inline(always)]
        unsafe fn and(self, vector2: Self) -> __m128i {
            _mm_and_si128(self, vector2)
        }
        #[inline(always)]
        unsafe fn or(self, vector2: Self) -> __m128i {
            _mm_or_si128(self, vector2)
        }
    }
}
#[cfg(target_arch = "x86_64")]
mod x86avx2 {
    use super::{SensibleMoveMask, Vector};
    use core::arch::x86_64::*;
    impl Vector for __m256i {
        const BYTES: usize = 32;
        const ALIGN: usize = Self::BYTES - 1;
        type Mask = SensibleMoveMask;
        #[inline(always)]
        unsafe fn splat(byte: u8) -> __m256i {
            _mm256_set1_epi8(byte as i8)
        }
        #[inline(always)]
        unsafe fn load_aligned(data: *const u8) -> __m256i {
            _mm256_load_si256(data as *const __m256i)
        }
        #[inline(always)]
        unsafe fn load_unaligned(data: *const u8) -> __m256i {
            _mm256_loadu_si256(data as *const __m256i)
        }
        #[inline(always)]
        unsafe fn movemask(self) -> SensibleMoveMask {
            SensibleMoveMask(_mm256_movemask_epi8(self) as u32)
        }
        #[inline(always)]
        unsafe fn cmpeq(self, vector2: Self) -> __m256i {
            _mm256_cmpeq_epi8(self, vector2)
        }
        #[inline(always)]
        unsafe fn and(self, vector2: Self) -> __m256i {
            _mm256_and_si256(self, vector2)
        }
        #[inline(always)]
        unsafe fn or(self, vector2: Self) -> __m256i {
            _mm256_or_si256(self, vector2)
        }
    }
}
#[cfg(target_arch = "aarch64")]
mod aarch64neon {
    use super::{MoveMask, Vector};
    use core::arch::aarch64::*;
    impl Vector for uint8x16_t {
        const BYTES: usize = 16;
        const ALIGN: usize = Self::BYTES - 1;
        type Mask = NeonMoveMask;
        #[inline(always)]
        unsafe fn splat(byte: u8) -> uint8x16_t {
            unsafe { vdupq_n_u8(byte) }
        }
        #[inline(always)]
        unsafe fn load_aligned(data: *const u8) -> uint8x16_t {
            unsafe { Self::load_unaligned(data) }
        }
        #[inline(always)]
        unsafe fn load_unaligned(data: *const u8) -> uint8x16_t {
            unsafe { vld1q_u8(data) }
        }
        #[inline(always)]
        #[cfg(target_endian = "little")]
        unsafe fn movemask(self) -> NeonMoveMask {
            unsafe {
                let asu16s = vreinterpretq_u16_u8(self);
                let mask = vshrn_n_u16(asu16s, 4);
                let asu64 = vreinterpret_u64_u8(mask);
                let scalar64 = vget_lane_u64(asu64, 0);
                NeonMoveMask(scalar64 & 0x8888888888888888)
            }
        }
        #[inline(always)]
        #[cfg(target_endian = "big")]
        unsafe fn movemask(self) -> NeonMoveMask {
            let asu16s = vreinterpretq_u16_u8(vrev16q_u8(self));
            let mask = vshrn_n_u16(asu16s, 4);
            let asu64 = vreinterpret_u64_u8(mask);
            let scalar64 = vget_lane_u64(asu64, 0).swap_bytes();
            NeonMoveMask(scalar64 & 0x8888888888888888)
        }
        #[inline(always)]
        unsafe fn cmpeq(self, vector2: Self) -> uint8x16_t {
            unsafe { vceqq_u8(self, vector2) }
        }
        #[inline(always)]
        unsafe fn and(self, vector2: Self) -> uint8x16_t {
            unsafe { vandq_u8(self, vector2) }
        }
        #[inline(always)]
        unsafe fn or(self, vector2: Self) -> uint8x16_t {
            unsafe { vorrq_u8(self, vector2) }
        }
        #[inline(always)]
        unsafe fn movemask_will_have_non_zero(self) -> bool {
            unsafe {
                let low = vreinterpretq_u64_u8(vpmaxq_u8(self, self));
                vgetq_lane_u64(low, 0) != 0
            }
        }
    }
    #[derive(Clone, Copy, Debug)]
    pub(crate) struct NeonMoveMask(u64);
    impl NeonMoveMask {
        #[inline(always)]
        fn get_for_offset(self) -> u64 {
            self.0
        }
    }
    impl MoveMask for NeonMoveMask {
        #[inline(always)]
        fn all_zeros_except_least_significant(n: usize) -> NeonMoveMask {
            debug_assert!(n < 16);
            NeonMoveMask(!(((1 << n) << 2) - 1))
        }
        #[inline(always)]
        fn has_non_zero(self) -> bool {
            self.0 != 0
        }
        #[inline(always)]
        fn count_ones(self) -> usize {
            self.0.count_ones() as usize
        }
        #[inline(always)]
        fn and(self, other: NeonMoveMask) -> NeonMoveMask {
            NeonMoveMask(self.0 & other.0)
        }
        #[inline(always)]
        fn or(self, other: NeonMoveMask) -> NeonMoveMask {
            NeonMoveMask(self.0 | other.0)
        }
        #[inline(always)]
        fn clear_least_significant_bit(self) -> NeonMoveMask {
            NeonMoveMask(self.0 & (self.0 - 1))
        }
        #[inline(always)]
        fn first_offset(self) -> usize {
            (self.get_for_offset().trailing_zeros() >> 2) as usize
        }
        #[inline(always)]
        fn last_offset(self) -> usize {
            16 - (self.get_for_offset().leading_zeros() >> 2) as usize - 1
        }
    }
}
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
mod wasm_simd128 {
    use super::{SensibleMoveMask, Vector};
    use core::arch::wasm32::*;
    impl Vector for v128 {
        const BYTES: usize = 16;
        const ALIGN: usize = Self::BYTES - 1;
        type Mask = SensibleMoveMask;
        #[inline(always)]
        unsafe fn splat(byte: u8) -> v128 {
            u8x16_splat(byte)
        }
        #[inline(always)]
        unsafe fn load_aligned(data: *const u8) -> v128 {
            *data.cast()
        }
        #[inline(always)]
        unsafe fn load_unaligned(data: *const u8) -> v128 {
            v128_load(data.cast())
        }
        #[inline(always)]
        unsafe fn movemask(self) -> SensibleMoveMask {
            SensibleMoveMask(u8x16_bitmask(self).into())
        }
        #[inline(always)]
        unsafe fn cmpeq(self, vector2: Self) -> v128 {
            u8x16_eq(self, vector2)
        }
        #[inline(always)]
        unsafe fn and(self, vector2: Self) -> v128 {
            v128_and(self, vector2)
        }
        #[inline(always)]
        unsafe fn or(self, vector2: Self) -> v128 {
            v128_or(self, vector2)
        }
    }
}
