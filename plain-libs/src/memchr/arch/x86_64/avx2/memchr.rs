use crate::memchr::{arch::generic::memchr as generic, ext::Pointer, vector::Vector};
use core::arch::x86_64::{__m128i, __m256i};
#[derive(Clone, Copy, Debug)]
pub struct One {
    sse2: generic::One<__m128i>,
    avx2: generic::One<__m256i>,
}
impl One {
    #[inline]
    pub fn new(needle: u8) -> Option<One> {
        if One::is_available() {
            unsafe { Some(One::new_unchecked(needle)) }
        } else {
            None
        }
    }
    #[target_feature(enable = "sse2", enable = "avx2")]
    #[inline]
    pub unsafe fn new_unchecked(needle: u8) -> One {
        One {
            sse2: generic::One::new(needle),
            avx2: generic::One::new(needle),
        }
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
    pub fn find(&self, haystack: &[u8]) -> Option<usize> {
        unsafe { generic::search_slice_with_raw(haystack, |s, e| self.find_raw(s, e)) }
    }
    #[inline]
    pub fn rfind(&self, haystack: &[u8]) -> Option<usize> {
        unsafe { generic::search_slice_with_raw(haystack, |s, e| self.rfind_raw(s, e)) }
    }
    #[inline]
    pub fn count(&self, haystack: &[u8]) -> usize {
        unsafe {
            let start = haystack.as_ptr();
            let end = start.add(haystack.len());
            self.count_raw(start, end)
        }
    }
    #[inline]
    pub unsafe fn find_raw(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        if start >= end {
            return None;
        }
        let len = end.distance(start);
        if len < __m256i::BYTES {
            return if len < __m128i::BYTES {
                generic::fwd_byte_by_byte(start, end, |b| b == self.sse2.needle1())
            } else {
                self.find_raw_sse2(start, end)
            };
        }
        self.find_raw_avx2(start, end)
    }
    #[inline]
    pub unsafe fn rfind_raw(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        if start >= end {
            return None;
        }
        let len = end.distance(start);
        if len < __m256i::BYTES {
            return if len < __m128i::BYTES {
                generic::rev_byte_by_byte(start, end, |b| b == self.sse2.needle1())
            } else {
                self.rfind_raw_sse2(start, end)
            };
        }
        self.rfind_raw_avx2(start, end)
    }
    #[inline]
    pub unsafe fn count_raw(&self, start: *const u8, end: *const u8) -> usize {
        if start >= end {
            return 0;
        }
        let len = end.distance(start);
        if len < __m256i::BYTES {
            return if len < __m128i::BYTES {
                generic::count_byte_by_byte(start, end, |b| b == self.sse2.needle1())
            } else {
                self.count_raw_sse2(start, end)
            };
        }
        self.count_raw_avx2(start, end)
    }
    #[target_feature(enable = "sse2")]
    #[inline]
    unsafe fn find_raw_sse2(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        self.sse2.find_raw(start, end)
    }
    #[target_feature(enable = "sse2")]
    #[inline]
    unsafe fn rfind_raw_sse2(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        self.sse2.rfind_raw(start, end)
    }
    #[target_feature(enable = "sse2")]
    #[inline]
    unsafe fn count_raw_sse2(&self, start: *const u8, end: *const u8) -> usize {
        self.sse2.count_raw(start, end)
    }
    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn find_raw_avx2(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        self.avx2.find_raw(start, end)
    }
    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn rfind_raw_avx2(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        self.avx2.rfind_raw(start, end)
    }
    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn count_raw_avx2(&self, start: *const u8, end: *const u8) -> usize {
        self.avx2.count_raw(start, end)
    }
    #[inline]
    pub fn iter<'a, 'h>(&'a self, haystack: &'h [u8]) -> OneIter<'a, 'h> {
        OneIter {
            searcher: self,
            it: generic::Iter::new(haystack),
        }
    }
}
#[derive(Clone, Debug)]
pub struct OneIter<'a, 'h> {
    searcher: &'a One,
    it: generic::Iter<'h>,
}
impl<'a, 'h> Iterator for OneIter<'a, 'h> {
    type Item = usize;
    #[inline]
    fn next(&mut self) -> Option<usize> {
        unsafe { self.it.next(|s, e| self.searcher.find_raw(s, e)) }
    }
    #[inline]
    fn count(self) -> usize {
        self.it
            .count(|s, e| unsafe { self.searcher.count_raw(s, e) })
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }
}
impl<'a, 'h> DoubleEndedIterator for OneIter<'a, 'h> {
    #[inline]
    fn next_back(&mut self) -> Option<usize> {
        unsafe { self.it.next_back(|s, e| self.searcher.rfind_raw(s, e)) }
    }
}
impl<'a, 'h> core::iter::FusedIterator for OneIter<'a, 'h> {}
#[derive(Clone, Copy, Debug)]
pub struct Two {
    sse2: generic::Two<__m128i>,
    avx2: generic::Two<__m256i>,
}
impl Two {
    #[inline]
    pub fn new(needle1: u8, needle2: u8) -> Option<Two> {
        if Two::is_available() {
            unsafe { Some(Two::new_unchecked(needle1, needle2)) }
        } else {
            None
        }
    }
    #[target_feature(enable = "sse2", enable = "avx2")]
    #[inline]
    pub unsafe fn new_unchecked(needle1: u8, needle2: u8) -> Two {
        Two {
            sse2: generic::Two::new(needle1, needle2),
            avx2: generic::Two::new(needle1, needle2),
        }
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
    pub fn find(&self, haystack: &[u8]) -> Option<usize> {
        unsafe { generic::search_slice_with_raw(haystack, |s, e| self.find_raw(s, e)) }
    }
    #[inline]
    pub fn rfind(&self, haystack: &[u8]) -> Option<usize> {
        unsafe { generic::search_slice_with_raw(haystack, |s, e| self.rfind_raw(s, e)) }
    }
    #[inline]
    pub unsafe fn find_raw(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        if start >= end {
            return None;
        }
        let len = end.distance(start);
        if len < __m256i::BYTES {
            return if len < __m128i::BYTES {
                generic::fwd_byte_by_byte(start, end, |b| {
                    b == self.sse2.needle1() || b == self.sse2.needle2()
                })
            } else {
                self.find_raw_sse2(start, end)
            };
        }
        self.find_raw_avx2(start, end)
    }
    #[inline]
    pub unsafe fn rfind_raw(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        if start >= end {
            return None;
        }
        let len = end.distance(start);
        if len < __m256i::BYTES {
            return if len < __m128i::BYTES {
                generic::rev_byte_by_byte(start, end, |b| {
                    b == self.sse2.needle1() || b == self.sse2.needle2()
                })
            } else {
                self.rfind_raw_sse2(start, end)
            };
        }
        self.rfind_raw_avx2(start, end)
    }
    #[target_feature(enable = "sse2")]
    #[inline]
    unsafe fn find_raw_sse2(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        self.sse2.find_raw(start, end)
    }
    #[target_feature(enable = "sse2")]
    #[inline]
    unsafe fn rfind_raw_sse2(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        self.sse2.rfind_raw(start, end)
    }
    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn find_raw_avx2(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        self.avx2.find_raw(start, end)
    }
    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn rfind_raw_avx2(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        self.avx2.rfind_raw(start, end)
    }
    #[inline]
    pub fn iter<'a, 'h>(&'a self, haystack: &'h [u8]) -> TwoIter<'a, 'h> {
        TwoIter {
            searcher: self,
            it: generic::Iter::new(haystack),
        }
    }
}
#[derive(Clone, Debug)]
pub struct TwoIter<'a, 'h> {
    searcher: &'a Two,
    it: generic::Iter<'h>,
}
impl<'a, 'h> Iterator for TwoIter<'a, 'h> {
    type Item = usize;
    #[inline]
    fn next(&mut self) -> Option<usize> {
        unsafe { self.it.next(|s, e| self.searcher.find_raw(s, e)) }
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }
}
impl<'a, 'h> DoubleEndedIterator for TwoIter<'a, 'h> {
    #[inline]
    fn next_back(&mut self) -> Option<usize> {
        unsafe { self.it.next_back(|s, e| self.searcher.rfind_raw(s, e)) }
    }
}
impl<'a, 'h> core::iter::FusedIterator for TwoIter<'a, 'h> {}
#[derive(Clone, Copy, Debug)]
pub struct Three {
    sse2: generic::Three<__m128i>,
    avx2: generic::Three<__m256i>,
}
impl Three {
    #[inline]
    pub fn new(needle1: u8, needle2: u8, needle3: u8) -> Option<Three> {
        if Three::is_available() {
            unsafe { Some(Three::new_unchecked(needle1, needle2, needle3)) }
        } else {
            None
        }
    }
    #[target_feature(enable = "sse2", enable = "avx2")]
    #[inline]
    pub unsafe fn new_unchecked(needle1: u8, needle2: u8, needle3: u8) -> Three {
        Three {
            sse2: generic::Three::new(needle1, needle2, needle3),
            avx2: generic::Three::new(needle1, needle2, needle3),
        }
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
    pub fn find(&self, haystack: &[u8]) -> Option<usize> {
        unsafe { generic::search_slice_with_raw(haystack, |s, e| self.find_raw(s, e)) }
    }
    #[inline]
    pub fn rfind(&self, haystack: &[u8]) -> Option<usize> {
        unsafe { generic::search_slice_with_raw(haystack, |s, e| self.rfind_raw(s, e)) }
    }
    #[inline]
    pub unsafe fn find_raw(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        if start >= end {
            return None;
        }
        let len = end.distance(start);
        if len < __m256i::BYTES {
            return if len < __m128i::BYTES {
                generic::fwd_byte_by_byte(start, end, |b| {
                    b == self.sse2.needle1() || b == self.sse2.needle2() || b == self.sse2.needle3()
                })
            } else {
                self.find_raw_sse2(start, end)
            };
        }
        self.find_raw_avx2(start, end)
    }
    #[inline]
    pub unsafe fn rfind_raw(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        if start >= end {
            return None;
        }
        let len = end.distance(start);
        if len < __m256i::BYTES {
            return if len < __m128i::BYTES {
                generic::rev_byte_by_byte(start, end, |b| {
                    b == self.sse2.needle1() || b == self.sse2.needle2() || b == self.sse2.needle3()
                })
            } else {
                self.rfind_raw_sse2(start, end)
            };
        }
        self.rfind_raw_avx2(start, end)
    }
    #[target_feature(enable = "sse2")]
    #[inline]
    unsafe fn find_raw_sse2(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        self.sse2.find_raw(start, end)
    }
    #[target_feature(enable = "sse2")]
    #[inline]
    unsafe fn rfind_raw_sse2(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        self.sse2.rfind_raw(start, end)
    }
    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn find_raw_avx2(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        self.avx2.find_raw(start, end)
    }
    #[target_feature(enable = "avx2")]
    #[inline]
    unsafe fn rfind_raw_avx2(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        self.avx2.rfind_raw(start, end)
    }
    #[inline]
    pub fn iter<'a, 'h>(&'a self, haystack: &'h [u8]) -> ThreeIter<'a, 'h> {
        ThreeIter {
            searcher: self,
            it: generic::Iter::new(haystack),
        }
    }
}
#[derive(Clone, Debug)]
pub struct ThreeIter<'a, 'h> {
    searcher: &'a Three,
    it: generic::Iter<'h>,
}
impl<'a, 'h> Iterator for ThreeIter<'a, 'h> {
    type Item = usize;
    #[inline]
    fn next(&mut self) -> Option<usize> {
        unsafe { self.it.next(|s, e| self.searcher.find_raw(s, e)) }
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }
}
impl<'a, 'h> DoubleEndedIterator for ThreeIter<'a, 'h> {
    #[inline]
    fn next_back(&mut self) -> Option<usize> {
        unsafe { self.it.next_back(|s, e| self.searcher.rfind_raw(s, e)) }
    }
}
impl<'a, 'h> core::iter::FusedIterator for ThreeIter<'a, 'h> {}
