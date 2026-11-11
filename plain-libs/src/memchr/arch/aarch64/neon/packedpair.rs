use crate::memchr::arch::{all::packedpair::Pair, generic::packedpair};
use core::arch::aarch64::uint8x16_t;
#[derive(Clone, Copy, Debug)]
pub struct Finder(packedpair::Finder<uint8x16_t>);
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
    #[target_feature(enable = "neon")]
    #[inline]
    unsafe fn with_pair_impl(needle: &[u8], pair: Pair) -> Finder {
        unsafe {
            let finder = packedpair::Finder::<uint8x16_t>::new(needle, pair);
            Finder(finder)
        }
    }
    #[inline]
    pub fn is_available() -> bool {
        #[cfg(target_feature = "neon")]
        {
            true
        }
        #[cfg(not(target_feature = "neon"))]
        {
            false
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
    #[target_feature(enable = "neon")]
    #[inline]
    unsafe fn find_impl(&self, haystack: &[u8], needle: &[u8]) -> Option<usize> {
        unsafe { self.0.find(haystack, needle) }
    }
    #[target_feature(enable = "neon")]
    #[inline]
    unsafe fn find_prefilter_impl(&self, haystack: &[u8]) -> Option<usize> {
        unsafe { self.0.find_prefilter(haystack) }
    }
    #[inline]
    pub fn pair(&self) -> &Pair {
        self.0.pair()
    }
    #[inline]
    pub fn min_haystack_len(&self) -> usize {
        self.0.min_haystack_len()
    }
}
