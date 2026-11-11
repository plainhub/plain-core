use crate::memchr::{
    arch::all::{is_equal_raw, packedpair::Pair},
    ext::Pointer,
    vector::{MoveMask, Vector},
};
#[derive(Clone, Copy, Debug)]
pub(crate) struct Finder<V> {
    pair: Pair,
    v1: V,
    v2: V,
    min_haystack_len: usize,
}
impl<V: Vector> Finder<V> {
    #[inline(always)]
    pub(crate) unsafe fn new(needle: &[u8], pair: Pair) -> Finder<V> {
        unsafe {
            let max_index = pair.index1().max(pair.index2());
            let min_haystack_len = core::cmp::max(needle.len(), usize::from(max_index) + V::BYTES);
            let v1 = V::splat(needle[usize::from(pair.index1())]);
            let v2 = V::splat(needle[usize::from(pair.index2())]);
            Finder {
                pair,
                v1,
                v2,
                min_haystack_len,
            }
        }
    }
    #[inline(always)]
    pub(crate) unsafe fn find(&self, haystack: &[u8], needle: &[u8]) -> Option<usize> {
        unsafe {
            assert!(
                haystack.len() >= self.min_haystack_len,
                "haystack too small, should be at least {} but got {}",
                self.min_haystack_len,
                haystack.len(),
            );
            let all = V::Mask::all_zeros_except_least_significant(0);
            let start = haystack.as_ptr();
            let end = start.add(haystack.len());
            let max = end.sub(self.min_haystack_len);
            let mut cur = start;
            if haystack.len() - self.min_haystack_len >= V::BYTES {
                let paired_max = max.sub(V::BYTES);
                while cur <= paired_max {
                    #[cfg(test)]
                    tests::record_paired_iteration();
                    if let Some(offset) = self.find_in_chunk(needle, cur, end, all) {
                        return Some(matched(start, cur, offset));
                    }
                    cur = cur.add(V::BYTES);
                    if let Some(offset) = self.find_in_chunk(needle, cur, end, all) {
                        return Some(matched(start, cur, offset));
                    }
                    cur = cur.add(V::BYTES);
                }
            }
            while cur <= max {
                if let Some(chunki) = self.find_in_chunk(needle, cur, end, all) {
                    return Some(matched(start, cur, chunki));
                }
                cur = cur.add(V::BYTES);
            }
            if cur < end {
                let remaining = end.distance(cur);
                debug_assert!(
                    remaining < self.min_haystack_len,
                    "remaining bytes should be smaller than the minimum haystack \
                 length of {}, but there are {} bytes remaining",
                    self.min_haystack_len,
                    remaining,
                );
                if remaining < needle.len() {
                    return None;
                }
                debug_assert!(max < cur, "after main loop, cur should have exceeded max",);
                let overlap = cur.distance(max);
                debug_assert!(overlap > 0, "overlap ({}) must always be non-zero", overlap,);
                debug_assert!(
                    overlap < V::BYTES,
                    "overlap ({}) cannot possibly be >= than a vector ({})",
                    overlap,
                    V::BYTES,
                );
                let mask = V::Mask::all_zeros_except_least_significant(overlap);
                cur = max;
                let m = self.find_in_chunk(needle, cur, end, mask);
                if let Some(chunki) = m {
                    return Some(matched(start, cur, chunki));
                }
            }
            None
        }
    }
    #[inline(always)]
    pub(crate) unsafe fn find_prefilter(&self, haystack: &[u8]) -> Option<usize> {
        unsafe {
            assert!(
                haystack.len() >= self.min_haystack_len,
                "haystack too small, should be at least {} but got {}",
                self.min_haystack_len,
                haystack.len(),
            );
            let start = haystack.as_ptr();
            let end = start.add(haystack.len());
            let max = end.sub(self.min_haystack_len);
            let mut cur = start;
            if haystack.len() - self.min_haystack_len >= V::BYTES {
                let paired_max = max.sub(V::BYTES);
                while cur <= paired_max {
                    #[cfg(test)]
                    tests::record_paired_iteration();
                    if let Some(offset) = self.find_prefilter_in_chunk(cur) {
                        return Some(matched(start, cur, offset));
                    }
                    cur = cur.add(V::BYTES);
                    if let Some(offset) = self.find_prefilter_in_chunk(cur) {
                        return Some(matched(start, cur, offset));
                    }
                    cur = cur.add(V::BYTES);
                }
            }
            while cur <= max {
                if let Some(chunki) = self.find_prefilter_in_chunk(cur) {
                    return Some(matched(start, cur, chunki));
                }
                cur = cur.add(V::BYTES);
            }
            if cur < end {
                cur = max;
                if let Some(chunki) = self.find_prefilter_in_chunk(cur) {
                    return Some(matched(start, cur, chunki));
                }
            }
            None
        }
    }
    #[inline(always)]
    unsafe fn find_in_chunk(
        &self,
        needle: &[u8],
        cur: *const u8,
        end: *const u8,
        mask: V::Mask,
    ) -> Option<usize> {
        unsafe {
            let index1 = usize::from(self.pair.index1());
            let index2 = usize::from(self.pair.index2());
            let chunk1 = V::load_unaligned(cur.add(index1));
            let chunk2 = V::load_unaligned(cur.add(index2));
            let eq1 = chunk1.cmpeq(self.v1);
            let eq2 = chunk2.cmpeq(self.v2);
            let mut offsets = eq1.and(eq2).movemask().and(mask);
            while offsets.has_non_zero() {
                let offset = offsets.first_offset();
                let cur = cur.add(offset);
                if end.distance(cur) < needle.len() {
                    return None;
                }
                if is_equal_raw(needle.as_ptr(), cur, needle.len()) {
                    return Some(offset);
                }
                offsets = offsets.clear_least_significant_bit();
            }
            None
        }
    }
    #[inline(always)]
    unsafe fn find_prefilter_in_chunk(&self, cur: *const u8) -> Option<usize> {
        unsafe {
            let index1 = usize::from(self.pair.index1());
            let index2 = usize::from(self.pair.index2());
            let chunk1 = V::load_unaligned(cur.add(index1));
            let chunk2 = V::load_unaligned(cur.add(index2));
            let eq1 = chunk1.cmpeq(self.v1);
            let eq2 = chunk2.cmpeq(self.v2);
            let offsets = eq1.and(eq2).movemask();
            if !offsets.has_non_zero() {
                return None;
            }
            Some(offsets.first_offset())
        }
    }
    #[inline]
    pub(crate) fn pair(&self) -> &Pair {
        &self.pair
    }
    #[inline(always)]
    pub(crate) fn min_haystack_len(&self) -> usize {
        self.min_haystack_len
    }
}
#[inline(always)]
unsafe fn matched(start: *const u8, cur: *const u8, chunki: usize) -> usize {
    unsafe { cur.distance(start) + chunki }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/memchr/arch/generic/packedpair.rs"]
mod tests;
