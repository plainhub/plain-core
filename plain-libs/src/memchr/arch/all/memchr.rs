use crate::memchr::{arch::generic::memchr as generic, ext::Pointer};
const USIZE_BYTES: usize = (usize::BITS / 8) as usize;
const USIZE_ALIGN: usize = USIZE_BYTES - 1;
#[derive(Clone, Copy, Debug)]
pub struct One {
    s1: u8,
    v1: usize,
}
impl One {
    const LOOP_BYTES: usize = 2 * USIZE_BYTES;
    #[inline]
    pub fn new(needle: u8) -> One {
        One {
            s1: needle,
            v1: splat(needle),
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
        unsafe {
            if start >= end {
                return None;
            }
            let confirm = |b| self.confirm(b);
            let len = end.distance(start);
            if len < USIZE_BYTES {
                return generic::fwd_byte_by_byte(start, end, confirm);
            }
            let chunk = start.cast::<usize>().read_unaligned();
            if self.has_needle(chunk) {
                return generic::fwd_byte_by_byte(start, end, confirm);
            }
            let mut cur = start.add(USIZE_BYTES - (start.as_usize() & USIZE_ALIGN));
            debug_assert!(cur > start);
            if len <= One::LOOP_BYTES {
                return generic::fwd_byte_by_byte(cur, end, confirm);
            }
            debug_assert!(end.sub(One::LOOP_BYTES) >= start);
            while cur <= end.sub(One::LOOP_BYTES) {
                debug_assert_eq!(0, cur.as_usize() % USIZE_BYTES);
                let a = cur.cast::<usize>().read();
                let b = cur.add(USIZE_BYTES).cast::<usize>().read();
                if self.has_needle(a) || self.has_needle(b) {
                    break;
                }
                cur = cur.add(One::LOOP_BYTES);
            }
            generic::fwd_byte_by_byte(cur, end, confirm)
        }
    }
    #[inline]
    pub unsafe fn rfind_raw(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        unsafe {
            if start >= end {
                return None;
            }
            let confirm = |b| self.confirm(b);
            let len = end.distance(start);
            if len < USIZE_BYTES {
                return generic::rev_byte_by_byte(start, end, confirm);
            }
            let chunk = end.sub(USIZE_BYTES).cast::<usize>().read_unaligned();
            if self.has_needle(chunk) {
                return generic::rev_byte_by_byte(start, end, confirm);
            }
            let mut cur = end.sub(end.as_usize() & USIZE_ALIGN);
            debug_assert!(start <= cur && cur <= end);
            if len <= One::LOOP_BYTES {
                return generic::rev_byte_by_byte(start, cur, confirm);
            }
            while cur >= start.add(One::LOOP_BYTES) {
                debug_assert_eq!(0, cur.as_usize() % USIZE_BYTES);
                let a = cur.sub(2 * USIZE_BYTES).cast::<usize>().read();
                let b = cur.sub(1 * USIZE_BYTES).cast::<usize>().read();
                if self.has_needle(a) || self.has_needle(b) {
                    break;
                }
                cur = cur.sub(One::LOOP_BYTES);
            }
            generic::rev_byte_by_byte(start, cur, confirm)
        }
    }
    #[inline]
    pub unsafe fn count_raw(&self, start: *const u8, end: *const u8) -> usize {
        unsafe {
            if start >= end {
                return 0;
            }
            let mut ptr = start;
            let mut count = 0;
            while ptr < end {
                count += (ptr.read() == self.s1) as usize;
                ptr = ptr.offset(1);
            }
            count
        }
    }
    pub fn iter<'a, 'h>(&'a self, haystack: &'h [u8]) -> OneIter<'a, 'h> {
        OneIter {
            searcher: self,
            it: generic::Iter::new(haystack),
        }
    }
    #[inline(always)]
    fn has_needle(&self, chunk: usize) -> bool {
        has_zero_byte(self.v1 ^ chunk)
    }
    #[inline(always)]
    fn confirm(&self, haystack_byte: u8) -> bool {
        self.s1 == haystack_byte
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
#[derive(Clone, Copy, Debug)]
pub struct Two {
    s1: u8,
    s2: u8,
    v1: usize,
    v2: usize,
}
impl Two {
    #[inline]
    pub fn new(needle1: u8, needle2: u8) -> Two {
        Two {
            s1: needle1,
            s2: needle2,
            v1: splat(needle1),
            v2: splat(needle2),
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
        unsafe {
            if start >= end {
                return None;
            }
            let confirm = |b| self.confirm(b);
            let len = end.distance(start);
            if len < USIZE_BYTES {
                return generic::fwd_byte_by_byte(start, end, confirm);
            }
            let chunk = start.cast::<usize>().read_unaligned();
            if self.has_needle(chunk) {
                return generic::fwd_byte_by_byte(start, end, confirm);
            }
            let mut cur = start.add(USIZE_BYTES - (start.as_usize() & USIZE_ALIGN));
            debug_assert!(cur > start);
            debug_assert!(end.sub(USIZE_BYTES) >= start);
            while cur <= end.sub(USIZE_BYTES) {
                debug_assert_eq!(0, cur.as_usize() % USIZE_BYTES);
                let chunk = cur.cast::<usize>().read();
                if self.has_needle(chunk) {
                    break;
                }
                cur = cur.add(USIZE_BYTES);
            }
            generic::fwd_byte_by_byte(cur, end, confirm)
        }
    }
    #[inline]
    pub unsafe fn rfind_raw(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        unsafe {
            if start >= end {
                return None;
            }
            let confirm = |b| self.confirm(b);
            let len = end.distance(start);
            if len < USIZE_BYTES {
                return generic::rev_byte_by_byte(start, end, confirm);
            }
            let chunk = end.sub(USIZE_BYTES).cast::<usize>().read_unaligned();
            if self.has_needle(chunk) {
                return generic::rev_byte_by_byte(start, end, confirm);
            }
            let mut cur = end.sub(end.as_usize() & USIZE_ALIGN);
            debug_assert!(start <= cur && cur <= end);
            while cur >= start.add(USIZE_BYTES) {
                debug_assert_eq!(0, cur.as_usize() % USIZE_BYTES);
                let chunk = cur.sub(USIZE_BYTES).cast::<usize>().read();
                if self.has_needle(chunk) {
                    break;
                }
                cur = cur.sub(USIZE_BYTES);
            }
            generic::rev_byte_by_byte(start, cur, confirm)
        }
    }
    pub fn iter<'a, 'h>(&'a self, haystack: &'h [u8]) -> TwoIter<'a, 'h> {
        TwoIter {
            searcher: self,
            it: generic::Iter::new(haystack),
        }
    }
    #[inline(always)]
    fn has_needle(&self, chunk: usize) -> bool {
        has_zero_byte(self.v1 ^ chunk) || has_zero_byte(self.v2 ^ chunk)
    }
    #[inline(always)]
    fn confirm(&self, haystack_byte: u8) -> bool {
        self.s1 == haystack_byte || self.s2 == haystack_byte
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
#[derive(Clone, Copy, Debug)]
pub struct Three {
    s1: u8,
    s2: u8,
    s3: u8,
    v1: usize,
    v2: usize,
    v3: usize,
}
impl Three {
    #[inline]
    pub fn new(needle1: u8, needle2: u8, needle3: u8) -> Three {
        Three {
            s1: needle1,
            s2: needle2,
            s3: needle3,
            v1: splat(needle1),
            v2: splat(needle2),
            v3: splat(needle3),
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
        unsafe {
            if start >= end {
                return None;
            }
            let confirm = |b| self.confirm(b);
            let len = end.distance(start);
            if len < USIZE_BYTES {
                return generic::fwd_byte_by_byte(start, end, confirm);
            }
            let chunk = start.cast::<usize>().read_unaligned();
            if self.has_needle(chunk) {
                return generic::fwd_byte_by_byte(start, end, confirm);
            }
            let mut cur = start.add(USIZE_BYTES - (start.as_usize() & USIZE_ALIGN));
            debug_assert!(cur > start);
            debug_assert!(end.sub(USIZE_BYTES) >= start);
            while cur <= end.sub(USIZE_BYTES) {
                debug_assert_eq!(0, cur.as_usize() % USIZE_BYTES);
                let chunk = cur.cast::<usize>().read();
                if self.has_needle(chunk) {
                    break;
                }
                cur = cur.add(USIZE_BYTES);
            }
            generic::fwd_byte_by_byte(cur, end, confirm)
        }
    }
    #[inline]
    pub unsafe fn rfind_raw(&self, start: *const u8, end: *const u8) -> Option<*const u8> {
        unsafe {
            if start >= end {
                return None;
            }
            let confirm = |b| self.confirm(b);
            let len = end.distance(start);
            if len < USIZE_BYTES {
                return generic::rev_byte_by_byte(start, end, confirm);
            }
            let chunk = end.sub(USIZE_BYTES).cast::<usize>().read_unaligned();
            if self.has_needle(chunk) {
                return generic::rev_byte_by_byte(start, end, confirm);
            }
            let mut cur = end.sub(end.as_usize() & USIZE_ALIGN);
            debug_assert!(start <= cur && cur <= end);
            while cur >= start.add(USIZE_BYTES) {
                debug_assert_eq!(0, cur.as_usize() % USIZE_BYTES);
                let chunk = cur.sub(USIZE_BYTES).cast::<usize>().read();
                if self.has_needle(chunk) {
                    break;
                }
                cur = cur.sub(USIZE_BYTES);
            }
            generic::rev_byte_by_byte(start, cur, confirm)
        }
    }
    pub fn iter<'a, 'h>(&'a self, haystack: &'h [u8]) -> ThreeIter<'a, 'h> {
        ThreeIter {
            searcher: self,
            it: generic::Iter::new(haystack),
        }
    }
    #[inline(always)]
    fn has_needle(&self, chunk: usize) -> bool {
        has_zero_byte(self.v1 ^ chunk)
            || has_zero_byte(self.v2 ^ chunk)
            || has_zero_byte(self.v3 ^ chunk)
    }
    #[inline(always)]
    fn confirm(&self, haystack_byte: u8) -> bool {
        self.s1 == haystack_byte || self.s2 == haystack_byte || self.s3 == haystack_byte
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
#[inline(always)]
fn has_zero_byte(x: usize) -> bool {
    const LO: usize = splat(0x01);
    const HI: usize = splat(0x80);
    (x.wrapping_sub(LO) & !x & HI) != 0
}
#[inline(always)]
const fn splat(b: u8) -> usize {
    (b as usize) * (usize::MAX / 255)
}
