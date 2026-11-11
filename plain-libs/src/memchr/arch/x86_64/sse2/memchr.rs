use crate::memchr::{arch::generic::memchr as generic, ext::Pointer, vector::Vector};
use core::arch::x86_64::__m128i;
#[derive(Clone, Copy, Debug)]
pub struct One(generic::One<__m128i>);
impl One {
    #[inline]
    pub fn new(needle: u8) -> Option<One> {
        if One::is_available() {
            unsafe { Some(One::new_unchecked(needle)) }
        } else {
            None
        }
    }
    #[target_feature(enable = "sse2")]
    #[inline]
    pub unsafe fn new_unchecked(needle: u8) -> One {
        One(generic::One::new(needle))
    }
    #[inline]
    pub fn is_available() -> bool {
        #[cfg(target_feature = "sse2")]
        {
            true
        }
        #[cfg(not(target_feature = "sse2"))]
        {
            false
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
        if end.distance(start) < __m128i::BYTES {
            return generic::fwd_byte_by_byte(start, end, |b| b == self.0.needle1());
        }
        self.find_raw_impl(start, end)
    }
    #[inline]
    pub unsafe fn rfind_raw(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        if start >= end {
            return None;
        }
        if end.distance(start) < __m128i::BYTES {
            return generic::rev_byte_by_byte(start, end, |b| b == self.0.needle1());
        }
        self.rfind_raw_impl(start, end)
    }
    #[inline]
    pub unsafe fn count_raw(&self, start: *const u8, end: *const u8) -> usize {
        if start >= end {
            return 0;
        }
        if end.distance(start) < __m128i::BYTES {
            return generic::count_byte_by_byte(start, end, |b| b == self.0.needle1());
        }
        self.count_raw_impl(start, end)
    }
    #[target_feature(enable = "sse2")]
    #[inline]
    unsafe fn find_raw_impl(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        self.0.find_raw(start, end)
    }
    #[target_feature(enable = "sse2")]
    #[inline]
    unsafe fn rfind_raw_impl(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        self.0.rfind_raw(start, end)
    }
    #[target_feature(enable = "sse2")]
    #[inline]
    unsafe fn count_raw_impl(&self, start: *const u8, end: *const u8) -> usize {
        self.0.count_raw(start, end)
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
pub struct Two(generic::Two<__m128i>);
impl Two {
    #[inline]
    pub fn new(needle1: u8, needle2: u8) -> Option<Two> {
        if Two::is_available() {
            unsafe { Some(Two::new_unchecked(needle1, needle2)) }
        } else {
            None
        }
    }
    #[target_feature(enable = "sse2")]
    #[inline]
    pub unsafe fn new_unchecked(needle1: u8, needle2: u8) -> Two {
        Two(generic::Two::new(needle1, needle2))
    }
    #[inline]
    pub fn is_available() -> bool {
        #[cfg(target_feature = "sse2")]
        {
            true
        }
        #[cfg(not(target_feature = "sse2"))]
        {
            false
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
        if end.distance(start) < __m128i::BYTES {
            return generic::fwd_byte_by_byte(start, end, |b| {
                b == self.0.needle1() || b == self.0.needle2()
            });
        }
        self.find_raw_impl(start, end)
    }
    #[inline]
    pub unsafe fn rfind_raw(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        if start >= end {
            return None;
        }
        if end.distance(start) < __m128i::BYTES {
            return generic::rev_byte_by_byte(start, end, |b| {
                b == self.0.needle1() || b == self.0.needle2()
            });
        }
        self.rfind_raw_impl(start, end)
    }
    #[target_feature(enable = "sse2")]
    #[inline]
    unsafe fn find_raw_impl(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        self.0.find_raw(start, end)
    }
    #[target_feature(enable = "sse2")]
    #[inline]
    unsafe fn rfind_raw_impl(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        self.0.rfind_raw(start, end)
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
pub struct Three(generic::Three<__m128i>);
impl Three {
    #[inline]
    pub fn new(needle1: u8, needle2: u8, needle3: u8) -> Option<Three> {
        if Three::is_available() {
            unsafe { Some(Three::new_unchecked(needle1, needle2, needle3)) }
        } else {
            None
        }
    }
    #[target_feature(enable = "sse2")]
    #[inline]
    pub unsafe fn new_unchecked(needle1: u8, needle2: u8, needle3: u8) -> Three {
        Three(generic::Three::new(needle1, needle2, needle3))
    }
    #[inline]
    pub fn is_available() -> bool {
        #[cfg(target_feature = "sse2")]
        {
            true
        }
        #[cfg(not(target_feature = "sse2"))]
        {
            false
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
        if end.distance(start) < __m128i::BYTES {
            return generic::fwd_byte_by_byte(start, end, |b| {
                b == self.0.needle1() || b == self.0.needle2() || b == self.0.needle3()
            });
        }
        self.find_raw_impl(start, end)
    }
    #[inline]
    pub unsafe fn rfind_raw(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        if start >= end {
            return None;
        }
        if end.distance(start) < __m128i::BYTES {
            return generic::rev_byte_by_byte(start, end, |b| {
                b == self.0.needle1() || b == self.0.needle2() || b == self.0.needle3()
            });
        }
        self.rfind_raw_impl(start, end)
    }
    #[target_feature(enable = "sse2")]
    #[inline]
    unsafe fn find_raw_impl(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        self.0.find_raw(start, end)
    }
    #[target_feature(enable = "sse2")]
    #[inline]
    unsafe fn rfind_raw_impl(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        self.0.rfind_raw(start, end)
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
