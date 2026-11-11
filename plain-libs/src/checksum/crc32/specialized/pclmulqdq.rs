#[cfg(target_arch = "x86")]
use core::arch::x86 as arch;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64 as arch;
#[derive(Clone, Copy)]
enum Kind {
    Sse,
    Avx2,
    Avx512,
}
#[derive(Clone)]
pub struct State {
    state: u32,
    kind: Kind,
}
impl State {
    fn detect() -> Option<Kind> {
        if is_x86_feature_detected!("pclmulqdq")
            && is_x86_feature_detected!("sse2")
            && is_x86_feature_detected!("sse4.1")
            && is_x86_feature_detected!("ssse3")
        {
            {
                if is_x86_feature_detected!("avx512f") && is_x86_feature_detected!("vpclmulqdq") {
                    return Some(Kind::Avx512);
                }
                if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("vpclmulqdq") {
                    return Some(Kind::Avx2);
                }
            }
            return Some(Kind::Sse);
        }
        None
    }
    pub fn new(state: u32) -> Option<Self> {
        Self::detect().map(|kind| Self { state, kind })
    }
    pub fn update(&mut self, buf: &[u8]) {
        self.state = unsafe {
            match self.kind {
                Kind::Sse => calculate(self.state, buf),
                Kind::Avx2 => calculate_avx2(self.state, buf),
                Kind::Avx512 => calculate_avx512(self.state, buf),
            }
        }
    }
    pub fn finalize(self) -> u32 {
        self.state
    }
    pub fn reset(&mut self) {
        self.state = 0;
    }
}
const K1: i64 = 0x154442bd4;
const K2: i64 = 0x1c6e41596;
const K3: i64 = 0x1751997d0;
const K4: i64 = 0x0ccaa009e;
const K5: i64 = 0x163cd6124;
const P_X: i64 = 0x1DB710641;
const U_PRIME: i64 = 0x1F7011641;
const MIN_FOLD_BY_4_BYTES: usize = 128;
const MIN_AVX512_BYTES: usize = 2 * 1024;
const K_1024_LOW: i64 = 0x1e88ef372;
const K_1024_HIGH: i64 = 0x14a7fe880;
const K_2048_LOW: i64 = 0x11542778a;
const K_2048_HIGH: i64 = 0x1322d1430;
#[target_feature(
    enable = "pclmulqdq",
    enable = "sse2",
    enable = "sse4.1",
    enable = "ssse3"
)]
unsafe fn calculate(crc: u32, mut data: &[u8]) -> u32 {
    unsafe {
        if data.len() < 16 {
            return crate::checksum::crc32::baseline::update_fast_16(crc, data);
        }
        if data.len() < MIN_FOLD_BY_4_BYTES {
            let mut x = get(&mut data);
            x = arch::_mm_xor_si128(x, arch::_mm_cvtsi32_si128(!crc as i32));
            return reduce_128_to_crc(x, data);
        }
        let mut x3 = get(&mut data);
        let mut x2 = get(&mut data);
        let mut x1 = get(&mut data);
        let mut x0 = get(&mut data);
        x3 = arch::_mm_xor_si128(x3, arch::_mm_cvtsi32_si128(!crc as i32));
        let k1k2 = arch::_mm_set_epi64x(K2, K1);
        while data.len() >= 64 {
            x3 = reduce128(x3, get(&mut data), k1k2);
            x2 = reduce128(x2, get(&mut data), k1k2);
            x1 = reduce128(x1, get(&mut data), k1k2);
            x0 = reduce128(x0, get(&mut data), k1k2);
        }
        let k3k4 = arch::_mm_set_epi64x(K4, K3);
        let mut x = reduce128(x3, x2, k3k4);
        x = reduce128(x, x1, k3k4);
        x = reduce128(x, x0, k3k4);
        reduce_128_to_crc(x, data)
    }
}
#[target_feature(
    enable = "pclmulqdq",
    enable = "sse2",
    enable = "sse4.1",
    enable = "ssse3",
    enable = "avx",
    enable = "avx2",
    enable = "vpclmulqdq"
)]
unsafe fn calculate_avx2(crc: u32, mut data: &[u8]) -> u32 {
    unsafe {
        if data.len() < 256 {
            return calculate(crc, data);
        }
        let mut v0 = get256(&mut data);
        let mut v1 = get256(&mut data);
        let mut v2 = get256(&mut data);
        let mut v3 = get256(&mut data);
        v0 = arch::_mm256_xor_si256(
            v0,
            arch::_mm256_castsi128_si256(arch::_mm_cvtsi32_si128(!crc as i32)),
        );
        let k = arch::_mm256_set_epi64x(K_1024_HIGH, K_1024_LOW, K_1024_HIGH, K_1024_LOW);
        while data.len() >= 128 {
            v0 = reduce256(v0, get256(&mut data), k);
            v1 = reduce256(v1, get256(&mut data), k);
            v2 = reduce256(v2, get256(&mut data), k);
            v3 = reduce256(v3, get256(&mut data), k);
        }
        let k3k4 = arch::_mm_set_epi64x(K4, K3);
        let mut x = arch::_mm256_castsi256_si128(v0);
        x = reduce128(x, arch::_mm256_extracti128_si256(v0, 1), k3k4);
        x = reduce128(x, arch::_mm256_castsi256_si128(v1), k3k4);
        x = reduce128(x, arch::_mm256_extracti128_si256(v1, 1), k3k4);
        x = reduce128(x, arch::_mm256_castsi256_si128(v2), k3k4);
        x = reduce128(x, arch::_mm256_extracti128_si256(v2, 1), k3k4);
        x = reduce128(x, arch::_mm256_castsi256_si128(v3), k3k4);
        x = reduce128(x, arch::_mm256_extracti128_si256(v3, 1), k3k4);
        reduce_128_to_crc(x, data)
    }
}
#[target_feature(
    enable = "pclmulqdq",
    enable = "sse2",
    enable = "sse4.1",
    enable = "ssse3",
    enable = "avx",
    enable = "avx2",
    enable = "vpclmulqdq",
    enable = "avx512f"
)]
unsafe fn calculate_avx512(crc: u32, mut data: &[u8]) -> u32 {
    unsafe {
        if data.len() < MIN_AVX512_BYTES {
            return calculate_avx2(crc, data);
        }
        let mut v0 = get512(&mut data);
        let mut v1 = get512(&mut data);
        let mut v2 = get512(&mut data);
        let mut v3 = get512(&mut data);
        v0 = arch::_mm512_xor_si512(
            v0,
            arch::_mm512_castsi128_si512(arch::_mm_cvtsi32_si128(!crc as i32)),
        );
        let k = arch::_mm512_set_epi64(
            K_2048_HIGH,
            K_2048_LOW,
            K_2048_HIGH,
            K_2048_LOW,
            K_2048_HIGH,
            K_2048_LOW,
            K_2048_HIGH,
            K_2048_LOW,
        );
        while data.len() >= 256 {
            v0 = reduce512(v0, get512(&mut data), k);
            v1 = reduce512(v1, get512(&mut data), k);
            v2 = reduce512(v2, get512(&mut data), k);
            v3 = reduce512(v3, get512(&mut data), k);
        }
        let k3k4 = arch::_mm_set_epi64x(K4, K3);
        let mut x = arch::_mm512_castsi512_si128(v0);
        x = reduce128(x, arch::_mm512_extracti32x4_epi32(v0, 1), k3k4);
        x = reduce128(x, arch::_mm512_extracti32x4_epi32(v0, 2), k3k4);
        x = reduce128(x, arch::_mm512_extracti32x4_epi32(v0, 3), k3k4);
        x = reduce128(x, arch::_mm512_castsi512_si128(v1), k3k4);
        x = reduce128(x, arch::_mm512_extracti32x4_epi32(v1, 1), k3k4);
        x = reduce128(x, arch::_mm512_extracti32x4_epi32(v1, 2), k3k4);
        x = reduce128(x, arch::_mm512_extracti32x4_epi32(v1, 3), k3k4);
        x = reduce128(x, arch::_mm512_castsi512_si128(v2), k3k4);
        x = reduce128(x, arch::_mm512_extracti32x4_epi32(v2, 1), k3k4);
        x = reduce128(x, arch::_mm512_extracti32x4_epi32(v2, 2), k3k4);
        x = reduce128(x, arch::_mm512_extracti32x4_epi32(v2, 3), k3k4);
        x = reduce128(x, arch::_mm512_castsi512_si128(v3), k3k4);
        x = reduce128(x, arch::_mm512_extracti32x4_epi32(v3, 1), k3k4);
        x = reduce128(x, arch::_mm512_extracti32x4_epi32(v3, 2), k3k4);
        x = reduce128(x, arch::_mm512_extracti32x4_epi32(v3, 3), k3k4);
        if data.len() >= MIN_FOLD_BY_4_BYTES {
            let crc = reduce_128_to_crc(x, &[]);
            return calculate(crc, data);
        }
        reduce_128_to_crc(x, data)
    }
}
#[target_feature(
    enable = "pclmulqdq",
    enable = "sse2",
    enable = "sse4.1",
    enable = "ssse3"
)]
unsafe fn reduce_128_to_crc(mut x: arch::__m128i, mut data: &[u8]) -> u32 {
    unsafe {
        let k3k4 = arch::_mm_set_epi64x(K4, K3);
        while data.len() >= 16 {
            x = reduce128(x, get(&mut data), k3k4);
        }
        let n = data.len();
        if n > 0 {
            let seq = arch::_mm_setr_epi8(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15);
            let shl = arch::_mm_add_epi8(seq, arch::_mm_set1_epi8(n as i8 - 16));
            let shr = arch::_mm_xor_si128(shl, arch::_mm_set1_epi8(-128));
            let overflow = arch::_mm_shuffle_epi8(x, shl);
            x = arch::_mm_shuffle_epi8(x, shr);
            let mut part = [0u8; 16];
            part[..n].copy_from_slice(data);
            let part = arch::_mm_loadu_si128(part.as_ptr() as *const arch::__m128i);
            x = arch::_mm_xor_si128(x, arch::_mm_shuffle_epi8(part, shl));
            x = reduce128(overflow, x, k3k4);
        }
        let x = arch::_mm_xor_si128(
            arch::_mm_clmulepi64_si128(x, k3k4, 0x10),
            arch::_mm_srli_si128(x, 8),
        );
        let x = arch::_mm_xor_si128(
            arch::_mm_clmulepi64_si128(
                arch::_mm_and_si128(x, arch::_mm_set_epi32(0, 0, 0, !0)),
                arch::_mm_set_epi64x(0, K5),
                0x00,
            ),
            arch::_mm_srli_si128(x, 4),
        );
        let pu = arch::_mm_set_epi64x(U_PRIME, P_X);
        let t1 = arch::_mm_clmulepi64_si128(
            arch::_mm_and_si128(x, arch::_mm_set_epi32(0, 0, 0, !0)),
            pu,
            0x10,
        );
        let t2 = arch::_mm_clmulepi64_si128(
            arch::_mm_and_si128(t1, arch::_mm_set_epi32(0, 0, 0, !0)),
            pu,
            0x00,
        );
        !(arch::_mm_extract_epi32(arch::_mm_xor_si128(x, t2), 1) as u32)
    }
}
#[inline]
unsafe fn reduce128(a: arch::__m128i, b: arch::__m128i, keys: arch::__m128i) -> arch::__m128i {
    unsafe {
        let t1 = arch::_mm_clmulepi64_si128(a, keys, 0x00);
        let t2 = arch::_mm_clmulepi64_si128(a, keys, 0x11);
        arch::_mm_xor_si128(arch::_mm_xor_si128(b, t1), t2)
    }
}
#[target_feature(enable = "avx2", enable = "vpclmulqdq")]
#[inline]
unsafe fn reduce256(a: arch::__m256i, b: arch::__m256i, keys: arch::__m256i) -> arch::__m256i {
    unsafe {
        let t1 = arch::_mm256_clmulepi64_epi128(a, keys, 0x00);
        let t2 = arch::_mm256_clmulepi64_epi128(a, keys, 0x11);
        arch::_mm256_xor_si256(arch::_mm256_xor_si256(b, t1), t2)
    }
}
#[target_feature(enable = "avx512f", enable = "vpclmulqdq")]
#[inline]
unsafe fn reduce512(a: arch::__m512i, b: arch::__m512i, keys: arch::__m512i) -> arch::__m512i {
    unsafe {
        let t1 = arch::_mm512_clmulepi64_epi128(a, keys, 0x00);
        let t2 = arch::_mm512_clmulepi64_epi128(a, keys, 0x11);
        arch::_mm512_xor_si512(arch::_mm512_xor_si512(b, t1), t2)
    }
}
unsafe fn get(a: &mut &[u8]) -> arch::__m128i {
    unsafe {
        debug_assert!(a.len() >= 16);
        let r = arch::_mm_loadu_si128(a.as_ptr() as *const arch::__m128i);
        *a = &a[16..];
        r
    }
}
#[target_feature(enable = "avx")]
#[inline]
unsafe fn get256(a: &mut &[u8]) -> arch::__m256i {
    unsafe {
        debug_assert!(a.len() >= 32);
        let r = arch::_mm256_loadu_si256(a.as_ptr() as *const arch::__m256i);
        *a = &a[32..];
        r
    }
}
#[target_feature(enable = "avx512f")]
#[inline]
unsafe fn get512(a: &mut &[u8]) -> arch::__m512i {
    unsafe {
        debug_assert!(a.len() >= 64);
        let r = arch::_mm512_loadu_si512(a.as_ptr() as *const _);
        *a = &a[64..];
        r
    }
}
