use crate::memchr::arch::{all::packedpair::Pair, generic::packedpair};
use core::arch::x86_64::{__m128i, __m256i};
#[derive(Clone, Copy, Debug)]
pub struct Finder {
    sse2: packedpair::Finder<__m128i>,
    avx2: packedpair::Finder<__m256i>,
}
impl Finder {
    #[inline]
    pub fn new(needle: &[u8]) -> Option<Finder> {
        Finder::with_pair(needle, Pair::new(needle)?)
    }
    #[inline]
    pub fn with_pair(needle: &[u8], pair: Pair) -> Option<Finder> {
        if Finder::is_available() {
            unsafe { Some(Finder::with_pair_impl(needle, pair)) }
        } else {
            None
        }
    }
    #[target_feature(enable = "sse2", enable = "avx2")]
    #[inline]
    unsafe fn with_pair_impl(needle: &[u8], pair: Pair) -> Finder {
        let sse2 = packedpair::Finder::<__m128i>::new(needle, pair);
        let avx2 = packedpair::Finder::<__m256i>::new(needle, pair);
        Finder { sse2, avx2 }
    }
    #[inline]
    pub fn is_available() -> bool {
        #[cfg(not(target_feature = "sse2"))]
        {
            false
        }
        #[cfg(target_feature = "sse2")]
        {
            #[cfg(target_feature = "avx2")]
            {
                true
            }
            #[cfg(not(target_feature = "avx2"))]
            {
                { std::is_x86_feature_detected!("avx2") }
            }
        }
    }
    #[inline]
    pub fn find(&self, haystack: &[u8], needle: &[u8]) -> Option<usize> {
        unsafe { self.find_impl(haystack, needle) }
    }
    #[inline]
    pub fn find_prefilter(&self, haystack: &[u8]) -> Option<usize> {
        unsafe { self.find_prefilter_impl(haystack) }
    }
    #[target_feature(enable = "sse2", enable = "avx2")]
    #[inline]
    unsafe fn find_impl(&self, haystack: &[u8], needle: &[u8]) -> Option<usize> {
        if haystack.len() < self.avx2.min_haystack_len() {
            self.sse2.find(haystack, needle)
        } else {
            self.avx2.find(haystack, needle)
        }
    }
    #[target_feature(enable = "sse2", enable = "avx2")]
    #[inline]
    unsafe fn find_prefilter_impl(&self, haystack: &[u8]) -> Option<usize> {
        if haystack.len() < self.avx2.min_haystack_len() {
            self.sse2.find_prefilter(haystack)
        } else {
            self.avx2.find_prefilter(haystack)
        }
    }
    #[inline]
    pub fn pair(&self) -> &Pair {
        self.avx2.pair()
    }
    #[inline]
    pub fn min_haystack_len(&self) -> usize {
        self.sse2.min_haystack_len()
    }
}
