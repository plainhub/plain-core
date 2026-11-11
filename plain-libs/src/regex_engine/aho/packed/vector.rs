use core::{
    fmt::Debug,
    panic::{RefUnwindSafe, UnwindSafe},
};
pub(crate) trait Vector: Copy + Debug + Send + Sync + UnwindSafe + RefUnwindSafe {
    const BITS: usize;
    const BYTES: usize;
    unsafe fn splat(byte: u8) -> Self;
    unsafe fn load_unaligned(data: *const u8) -> Self;
    unsafe fn is_zero(self) -> bool;
    unsafe fn cmpeq(self, vector2: Self) -> Self;
    unsafe fn and(self, vector2: Self) -> Self;
    unsafe fn or(self, vector2: Self) -> Self;
    unsafe fn shift_8bit_lane_right<const BITS: i32>(self) -> Self;
    unsafe fn shift_in_one_byte(self, vector2: Self) -> Self;
    unsafe fn shift_in_two_bytes(self, vector2: Self) -> Self;
    unsafe fn shift_in_three_bytes(self, vector2: Self) -> Self;
    unsafe fn shuffle_bytes(self, indices: Self) -> Self;
    unsafe fn for_each_64bit_lane<T>(self, f: impl FnMut(usize, u64) -> Option<T>) -> Option<T>;
}
pub(crate) trait FatVector: Vector {
    type Half: Vector;
    unsafe fn load_half_unaligned(data: *const u8) -> Self;
    unsafe fn half_shift_in_one_byte(self, vector2: Self) -> Self;
    unsafe fn half_shift_in_two_bytes(self, vector2: Self) -> Self;
    unsafe fn half_shift_in_three_bytes(self, vector2: Self) -> Self;
    unsafe fn swap_halves(self) -> Self;
    unsafe fn interleave_low_8bit_lanes(self, vector2: Self) -> Self;
    unsafe fn interleave_high_8bit_lanes(self, vector2: Self) -> Self;
    unsafe fn for_each_low_64bit_lane<T>(
        self,
        vector2: Self,
        f: impl FnMut(usize, u64) -> Option<T>,
    ) -> Option<T>;
}
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
mod x86_64_ssse3 {
    use super::Vector;
    use crate::regex_engine::aho::util::int::{I8, I32};
    use core::arch::x86_64::*;
    impl Vector for __m128i {
        const BITS: usize = 128;
        const BYTES: usize = 16;
        #[inline(always)]
        unsafe fn splat(byte: u8) -> __m128i {
            unsafe { _mm_set1_epi8(i8::from_bits(byte)) }
        }
        #[inline(always)]
        unsafe fn load_unaligned(data: *const u8) -> __m128i {
            unsafe { _mm_loadu_si128(data.cast::<__m128i>()) }
        }
        #[inline(always)]
        unsafe fn is_zero(self) -> bool {
            unsafe {
                let cmp = self.cmpeq(Self::splat(0));
                _mm_movemask_epi8(cmp).to_bits() == 0xFFFF
            }
        }
        #[inline(always)]
        unsafe fn cmpeq(self, vector2: Self) -> __m128i {
            unsafe { _mm_cmpeq_epi8(self, vector2) }
        }
        #[inline(always)]
        unsafe fn and(self, vector2: Self) -> __m128i {
            unsafe { _mm_and_si128(self, vector2) }
        }
        #[inline(always)]
        unsafe fn or(self, vector2: Self) -> __m128i {
            unsafe { _mm_or_si128(self, vector2) }
        }
        #[inline(always)]
        unsafe fn shift_8bit_lane_right<const BITS: i32>(self) -> Self {
            unsafe {
                let lomask = Self::splat(0xF);
                _mm_srli_epi16(self, BITS).and(lomask)
            }
        }
        #[inline(always)]
        unsafe fn shift_in_one_byte(self, vector2: Self) -> Self {
            unsafe { _mm_alignr_epi8(self, vector2, 15) }
        }
        #[inline(always)]
        unsafe fn shift_in_two_bytes(self, vector2: Self) -> Self {
            unsafe { _mm_alignr_epi8(self, vector2, 14) }
        }
        #[inline(always)]
        unsafe fn shift_in_three_bytes(self, vector2: Self) -> Self {
            unsafe { _mm_alignr_epi8(self, vector2, 13) }
        }
        #[inline(always)]
        unsafe fn shuffle_bytes(self, indices: Self) -> Self {
            unsafe { _mm_shuffle_epi8(self, indices) }
        }
        #[inline(always)]
        unsafe fn for_each_64bit_lane<T>(
            self,
            mut f: impl FnMut(usize, u64) -> Option<T>,
        ) -> Option<T> {
            unsafe {
                let lanes: [u64; 2] = core::mem::transmute(self);
                if let Some(t) = f(0, lanes[0]) {
                    return Some(t);
                }
                if let Some(t) = f(1, lanes[1]) {
                    return Some(t);
                }
                None
            }
        }
    }
}
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
mod x86_64_avx2 {
    use super::{FatVector, Vector};
    use crate::regex_engine::aho::util::int::{I8, I32, I64};
    use core::arch::x86_64::*;
    impl Vector for __m256i {
        const BITS: usize = 256;
        const BYTES: usize = 32;
        #[inline(always)]
        unsafe fn splat(byte: u8) -> __m256i {
            unsafe { _mm256_set1_epi8(i8::from_bits(byte)) }
        }
        #[inline(always)]
        unsafe fn load_unaligned(data: *const u8) -> __m256i {
            unsafe { _mm256_loadu_si256(data.cast::<__m256i>()) }
        }
        #[inline(always)]
        unsafe fn is_zero(self) -> bool {
            unsafe {
                let cmp = self.cmpeq(Self::splat(0));
                _mm256_movemask_epi8(cmp).to_bits() == 0xFFFFFFFF
            }
        }
        #[inline(always)]
        unsafe fn cmpeq(self, vector2: Self) -> __m256i {
            unsafe { _mm256_cmpeq_epi8(self, vector2) }
        }
        #[inline(always)]
        unsafe fn and(self, vector2: Self) -> __m256i {
            unsafe { _mm256_and_si256(self, vector2) }
        }
        #[inline(always)]
        unsafe fn or(self, vector2: Self) -> __m256i {
            unsafe { _mm256_or_si256(self, vector2) }
        }
        #[inline(always)]
        unsafe fn shift_8bit_lane_right<const BITS: i32>(self) -> Self {
            unsafe {
                let lomask = Self::splat(0xF);
                _mm256_srli_epi16(self, BITS).and(lomask)
            }
        }
        #[inline(always)]
        unsafe fn shift_in_one_byte(self, vector2: Self) -> Self {
            unsafe {
                let v = _mm256_permute2x128_si256(vector2, self, 0x21);
                _mm256_alignr_epi8(self, v, 15)
            }
        }
        #[inline(always)]
        unsafe fn shift_in_two_bytes(self, vector2: Self) -> Self {
            unsafe {
                let v = _mm256_permute2x128_si256(vector2, self, 0x21);
                _mm256_alignr_epi8(self, v, 14)
            }
        }
        #[inline(always)]
        unsafe fn shift_in_three_bytes(self, vector2: Self) -> Self {
            unsafe {
                let v = _mm256_permute2x128_si256(vector2, self, 0x21);
                _mm256_alignr_epi8(self, v, 13)
            }
        }
        #[inline(always)]
        unsafe fn shuffle_bytes(self, indices: Self) -> Self {
            unsafe { _mm256_shuffle_epi8(self, indices) }
        }
        #[inline(always)]
        unsafe fn for_each_64bit_lane<T>(
            self,
            mut f: impl FnMut(usize, u64) -> Option<T>,
        ) -> Option<T> {
            unsafe {
                let lane = _mm256_extract_epi64(self, 0).to_bits();
                if let Some(t) = f(0, lane) {
                    return Some(t);
                }
                let lane = _mm256_extract_epi64(self, 1).to_bits();
                if let Some(t) = f(1, lane) {
                    return Some(t);
                }
                let lane = _mm256_extract_epi64(self, 2).to_bits();
                if let Some(t) = f(2, lane) {
                    return Some(t);
                }
                let lane = _mm256_extract_epi64(self, 3).to_bits();
                if let Some(t) = f(3, lane) {
                    return Some(t);
                }
                None
            }
        }
    }
    impl FatVector for __m256i {
        type Half = __m128i;
        #[inline(always)]
        unsafe fn load_half_unaligned(data: *const u8) -> Self {
            let half = Self::Half::load_unaligned(data);
            _mm256_broadcastsi128_si256(half)
        }
        #[inline(always)]
        unsafe fn half_shift_in_one_byte(self, vector2: Self) -> Self {
            _mm256_alignr_epi8(self, vector2, 15)
        }
        #[inline(always)]
        unsafe fn half_shift_in_two_bytes(self, vector2: Self) -> Self {
            _mm256_alignr_epi8(self, vector2, 14)
        }
        #[inline(always)]
        unsafe fn half_shift_in_three_bytes(self, vector2: Self) -> Self {
            _mm256_alignr_epi8(self, vector2, 13)
        }
        #[inline(always)]
        unsafe fn swap_halves(self) -> Self {
            _mm256_permute4x64_epi64(self, 0x4E)
        }
        #[inline(always)]
        unsafe fn interleave_low_8bit_lanes(self, vector2: Self) -> Self {
            _mm256_unpacklo_epi8(self, vector2)
        }
        #[inline(always)]
        unsafe fn interleave_high_8bit_lanes(self, vector2: Self) -> Self {
            _mm256_unpackhi_epi8(self, vector2)
        }
        #[inline(always)]
        unsafe fn for_each_low_64bit_lane<T>(
            self,
            vector2: Self,
            mut f: impl FnMut(usize, u64) -> Option<T>,
        ) -> Option<T> {
            let lane = _mm256_extract_epi64(self, 0).to_bits();
            if let Some(t) = f(0, lane) {
                return Some(t);
            }
            let lane = _mm256_extract_epi64(self, 1).to_bits();
            if let Some(t) = f(1, lane) {
                return Some(t);
            }
            let lane = _mm256_extract_epi64(vector2, 0).to_bits();
            if let Some(t) = f(2, lane) {
                return Some(t);
            }
            let lane = _mm256_extract_epi64(vector2, 1).to_bits();
            if let Some(t) = f(3, lane) {
                return Some(t);
            }
            None
        }
    }
}
#[cfg(all(
    target_arch = "aarch64",
    target_feature = "neon",
    target_endian = "little"
))]
mod aarch64_neon {
    use super::Vector;
    use core::arch::aarch64::*;
    impl Vector for uint8x16_t {
        const BITS: usize = 128;
        const BYTES: usize = 16;
        #[inline(always)]
        unsafe fn splat(byte: u8) -> uint8x16_t {
            unsafe { vdupq_n_u8(byte) }
        }
        #[inline(always)]
        unsafe fn load_unaligned(data: *const u8) -> uint8x16_t {
            unsafe { vld1q_u8(data) }
        }
        #[inline(always)]
        unsafe fn is_zero(self) -> bool {
            unsafe {
                let maxes = vreinterpretq_u64_u8(vpmaxq_u8(self, self));
                vgetq_lane_u64(maxes, 0) == 0
            }
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
        unsafe fn shift_8bit_lane_right<const BITS: i32>(self) -> Self {
            unsafe {
                debug_assert!(BITS <= 7);
                vshrq_n_u8(self, BITS)
            }
        }
        #[inline(always)]
        unsafe fn shift_in_one_byte(self, vector2: Self) -> Self {
            unsafe { vextq_u8(vector2, self, 15) }
        }
        #[inline(always)]
        unsafe fn shift_in_two_bytes(self, vector2: Self) -> Self {
            unsafe { vextq_u8(vector2, self, 14) }
        }
        #[inline(always)]
        unsafe fn shift_in_three_bytes(self, vector2: Self) -> Self {
            unsafe { vextq_u8(vector2, self, 13) }
        }
        #[inline(always)]
        unsafe fn shuffle_bytes(self, indices: Self) -> Self {
            unsafe { vqtbl1q_u8(self, indices) }
        }
        #[inline(always)]
        unsafe fn for_each_64bit_lane<T>(
            self,
            mut f: impl FnMut(usize, u64) -> Option<T>,
        ) -> Option<T> {
            unsafe {
                let this = vreinterpretq_u64_u8(self);
                let lane = vgetq_lane_u64(this, 0);
                if let Some(t) = f(0, lane) {
                    return Some(t);
                }
                let lane = vgetq_lane_u64(this, 1);
                if let Some(t) = f(1, lane) {
                    return Some(t);
                }
                None
            }
        }
    }
}
