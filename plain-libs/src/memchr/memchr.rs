use crate::memchr::arch::generic::memchr as generic;
use core::iter::Rev;
#[inline]
pub fn memchr(needle: u8, haystack: &[u8]) -> Option<usize> {
    unsafe { generic::search_slice_with_raw(haystack, |start, end| memchr_raw(needle, start, end)) }
}
#[inline]
pub fn memrchr(needle: u8, haystack: &[u8]) -> Option<usize> {
    unsafe {
        generic::search_slice_with_raw(haystack, |start, end| memrchr_raw(needle, start, end))
    }
}
#[inline]
pub fn memchr2(needle1: u8, needle2: u8, haystack: &[u8]) -> Option<usize> {
    unsafe {
        generic::search_slice_with_raw(haystack, |start, end| {
            memchr2_raw(needle1, needle2, start, end)
        })
    }
}
#[inline]
pub fn memrchr2(needle1: u8, needle2: u8, haystack: &[u8]) -> Option<usize> {
    unsafe {
        generic::search_slice_with_raw(haystack, |start, end| {
            memrchr2_raw(needle1, needle2, start, end)
        })
    }
}
#[inline]
pub fn memchr3(needle1: u8, needle2: u8, needle3: u8, haystack: &[u8]) -> Option<usize> {
    unsafe {
        generic::search_slice_with_raw(haystack, |start, end| {
            memchr3_raw(needle1, needle2, needle3, start, end)
        })
    }
}
#[inline]
pub fn memrchr3(needle1: u8, needle2: u8, needle3: u8, haystack: &[u8]) -> Option<usize> {
    unsafe {
        generic::search_slice_with_raw(haystack, |start, end| {
            memrchr3_raw(needle1, needle2, needle3, start, end)
        })
    }
}
#[inline]
pub fn memchr_iter<'h>(needle: u8, haystack: &'h [u8]) -> Memchr<'h> {
    Memchr::new(needle, haystack)
}
#[inline]
pub fn memrchr_iter(needle: u8, haystack: &[u8]) -> Rev<Memchr<'_>> {
    Memchr::new(needle, haystack).rev()
}
#[inline]
pub fn memchr2_iter<'h>(needle1: u8, needle2: u8, haystack: &'h [u8]) -> Memchr2<'h> {
    Memchr2::new(needle1, needle2, haystack)
}
#[inline]
pub fn memrchr2_iter(needle1: u8, needle2: u8, haystack: &[u8]) -> Rev<Memchr2<'_>> {
    Memchr2::new(needle1, needle2, haystack).rev()
}
#[inline]
pub fn memchr3_iter<'h>(needle1: u8, needle2: u8, needle3: u8, haystack: &'h [u8]) -> Memchr3<'h> {
    Memchr3::new(needle1, needle2, needle3, haystack)
}
#[inline]
pub fn memrchr3_iter(needle1: u8, needle2: u8, needle3: u8, haystack: &[u8]) -> Rev<Memchr3<'_>> {
    Memchr3::new(needle1, needle2, needle3, haystack).rev()
}
#[derive(Clone, Debug)]
pub struct Memchr<'h> {
    needle1: u8,
    it: crate::memchr::arch::generic::memchr::Iter<'h>,
}
impl<'h> Memchr<'h> {
    #[inline]
    pub fn new(needle1: u8, haystack: &'h [u8]) -> Memchr<'h> {
        Memchr {
            needle1,
            it: crate::memchr::arch::generic::memchr::Iter::new(haystack),
        }
    }
}
impl<'h> Iterator for Memchr<'h> {
    type Item = usize;
    #[inline]
    fn next(&mut self) -> Option<usize> {
        unsafe { self.it.next(|s, e| memchr_raw(self.needle1, s, e)) }
    }
    #[inline]
    fn count(self) -> usize {
        self.it
            .count(|s, e| unsafe { count_raw(self.needle1, s, e) })
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }
}
impl<'h> DoubleEndedIterator for Memchr<'h> {
    #[inline]
    fn next_back(&mut self) -> Option<usize> {
        unsafe { self.it.next_back(|s, e| memrchr_raw(self.needle1, s, e)) }
    }
}
impl<'h> core::iter::FusedIterator for Memchr<'h> {}
#[derive(Clone, Debug)]
pub struct Memchr2<'h> {
    needle1: u8,
    needle2: u8,
    it: crate::memchr::arch::generic::memchr::Iter<'h>,
}
impl<'h> Memchr2<'h> {
    #[inline]
    pub fn new(needle1: u8, needle2: u8, haystack: &'h [u8]) -> Memchr2<'h> {
        Memchr2 {
            needle1,
            needle2,
            it: crate::memchr::arch::generic::memchr::Iter::new(haystack),
        }
    }
}
impl<'h> Iterator for Memchr2<'h> {
    type Item = usize;
    #[inline]
    fn next(&mut self) -> Option<usize> {
        unsafe {
            self.it
                .next(|s, e| memchr2_raw(self.needle1, self.needle2, s, e))
        }
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }
}
impl<'h> DoubleEndedIterator for Memchr2<'h> {
    #[inline]
    fn next_back(&mut self) -> Option<usize> {
        unsafe {
            self.it
                .next_back(|s, e| memrchr2_raw(self.needle1, self.needle2, s, e))
        }
    }
}
impl<'h> core::iter::FusedIterator for Memchr2<'h> {}
#[derive(Clone, Debug)]
pub struct Memchr3<'h> {
    needle1: u8,
    needle2: u8,
    needle3: u8,
    it: crate::memchr::arch::generic::memchr::Iter<'h>,
}
impl<'h> Memchr3<'h> {
    #[inline]
    pub fn new(needle1: u8, needle2: u8, needle3: u8, haystack: &'h [u8]) -> Memchr3<'h> {
        Memchr3 {
            needle1,
            needle2,
            needle3,
            it: crate::memchr::arch::generic::memchr::Iter::new(haystack),
        }
    }
}
impl<'h> Iterator for Memchr3<'h> {
    type Item = usize;
    #[inline]
    fn next(&mut self) -> Option<usize> {
        unsafe {
            self.it
                .next(|s, e| memchr3_raw(self.needle1, self.needle2, self.needle3, s, e))
        }
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }
}
impl<'h> DoubleEndedIterator for Memchr3<'h> {
    #[inline]
    fn next_back(&mut self) -> Option<usize> {
        unsafe {
            self.it
                .next_back(|s, e| memrchr3_raw(self.needle1, self.needle2, self.needle3, s, e))
        }
    }
}
impl<'h> core::iter::FusedIterator for Memchr3<'h> {}
#[inline]
unsafe fn memchr_raw(needle: u8, start: *const u8, end: *const u8) -> Option<*const u8> {
    unsafe {
        #[cfg(target_arch = "x86_64")]
        {
            crate::memchr::arch::x86_64::memchr::memchr_raw(needle, start, end)
        }
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        {
            crate::memchr::arch::wasm32::memchr::memchr_raw(needle, start, end)
        }
        #[cfg(target_arch = "aarch64")]
        {
            crate::memchr::arch::aarch64::memchr::memchr_raw(needle, start, end)
        }
        #[cfg(not(any(
            target_arch = "x86_64",
            all(target_arch = "wasm32", target_feature = "simd128"),
            target_arch = "aarch64"
        )))]
        {
            crate::memchr::arch::all::memchr::One::new(needle).find_raw(start, end)
        }
    }
}
#[inline]
unsafe fn memrchr_raw(needle: u8, start: *const u8, end: *const u8) -> Option<*const u8> {
    unsafe {
        #[cfg(target_arch = "x86_64")]
        {
            crate::memchr::arch::x86_64::memchr::memrchr_raw(needle, start, end)
        }
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        {
            crate::memchr::arch::wasm32::memchr::memrchr_raw(needle, start, end)
        }
        #[cfg(target_arch = "aarch64")]
        {
            crate::memchr::arch::aarch64::memchr::memrchr_raw(needle, start, end)
        }
        #[cfg(not(any(
            target_arch = "x86_64",
            all(target_arch = "wasm32", target_feature = "simd128"),
            target_arch = "aarch64"
        )))]
        {
            crate::memchr::arch::all::memchr::One::new(needle).rfind_raw(start, end)
        }
    }
}
#[inline]
unsafe fn memchr2_raw(
    needle1: u8,
    needle2: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    unsafe {
        #[cfg(target_arch = "x86_64")]
        {
            crate::memchr::arch::x86_64::memchr::memchr2_raw(needle1, needle2, start, end)
        }
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        {
            crate::memchr::arch::wasm32::memchr::memchr2_raw(needle1, needle2, start, end)
        }
        #[cfg(target_arch = "aarch64")]
        {
            crate::memchr::arch::aarch64::memchr::memchr2_raw(needle1, needle2, start, end)
        }
        #[cfg(not(any(
            target_arch = "x86_64",
            all(target_arch = "wasm32", target_feature = "simd128"),
            target_arch = "aarch64"
        )))]
        {
            crate::memchr::arch::all::memchr::Two::new(needle1, needle2).find_raw(start, end)
        }
    }
}
#[inline]
unsafe fn memrchr2_raw(
    needle1: u8,
    needle2: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    unsafe {
        #[cfg(target_arch = "x86_64")]
        {
            crate::memchr::arch::x86_64::memchr::memrchr2_raw(needle1, needle2, start, end)
        }
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        {
            crate::memchr::arch::wasm32::memchr::memrchr2_raw(needle1, needle2, start, end)
        }
        #[cfg(target_arch = "aarch64")]
        {
            crate::memchr::arch::aarch64::memchr::memrchr2_raw(needle1, needle2, start, end)
        }
        #[cfg(not(any(
            target_arch = "x86_64",
            all(target_arch = "wasm32", target_feature = "simd128"),
            target_arch = "aarch64"
        )))]
        {
            crate::memchr::arch::all::memchr::Two::new(needle1, needle2).rfind_raw(start, end)
        }
    }
}
#[inline]
unsafe fn memchr3_raw(
    needle1: u8,
    needle2: u8,
    needle3: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    unsafe {
        #[cfg(target_arch = "x86_64")]
        {
            crate::memchr::arch::x86_64::memchr::memchr3_raw(needle1, needle2, needle3, start, end)
        }
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        {
            crate::memchr::arch::wasm32::memchr::memchr3_raw(needle1, needle2, needle3, start, end)
        }
        #[cfg(target_arch = "aarch64")]
        {
            crate::memchr::arch::aarch64::memchr::memchr3_raw(needle1, needle2, needle3, start, end)
        }
        #[cfg(not(any(
            target_arch = "x86_64",
            all(target_arch = "wasm32", target_feature = "simd128"),
            target_arch = "aarch64"
        )))]
        {
            crate::memchr::arch::all::memchr::Three::new(needle1, needle2, needle3)
                .find_raw(start, end)
        }
    }
}
#[inline]
unsafe fn memrchr3_raw(
    needle1: u8,
    needle2: u8,
    needle3: u8,
    start: *const u8,
    end: *const u8,
) -> Option<*const u8> {
    unsafe {
        #[cfg(target_arch = "x86_64")]
        {
            crate::memchr::arch::x86_64::memchr::memrchr3_raw(needle1, needle2, needle3, start, end)
        }
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        {
            crate::memchr::arch::wasm32::memchr::memrchr3_raw(needle1, needle2, needle3, start, end)
        }
        #[cfg(target_arch = "aarch64")]
        {
            crate::memchr::arch::aarch64::memchr::memrchr3_raw(
                needle1, needle2, needle3, start, end,
            )
        }
        #[cfg(not(any(
            target_arch = "x86_64",
            all(target_arch = "wasm32", target_feature = "simd128"),
            target_arch = "aarch64"
        )))]
        {
            crate::memchr::arch::all::memchr::Three::new(needle1, needle2, needle3)
                .rfind_raw(start, end)
        }
    }
}
#[inline]
unsafe fn count_raw(needle: u8, start: *const u8, end: *const u8) -> usize {
    unsafe {
        #[cfg(target_arch = "x86_64")]
        {
            crate::memchr::arch::x86_64::memchr::count_raw(needle, start, end)
        }
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        {
            crate::memchr::arch::wasm32::memchr::count_raw(needle, start, end)
        }
        #[cfg(target_arch = "aarch64")]
        {
            crate::memchr::arch::aarch64::memchr::count_raw(needle, start, end)
        }
        #[cfg(not(any(
            target_arch = "x86_64",
            all(target_arch = "wasm32", target_feature = "simd128"),
            target_arch = "aarch64"
        )))]
        {
            crate::memchr::arch::all::memchr::One::new(needle).count_raw(start, end)
        }
    }
}
