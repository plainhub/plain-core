use crate::regex_engine::aho::{
    PatternID,
    packed::{
        pattern::Patterns,
        vector::{FatVector, Vector},
    },
};
use crate::{memchr::ext::Pointer, regex_engine::int::U32};
use core::fmt::Debug;
use std::{boxed::Box, collections::BTreeMap, format, sync::Arc, vec, vec::Vec};
#[derive(Clone, Copy, Debug)]
pub(crate) struct Match {
    pid: PatternID,
    start: *const u8,
    end: *const u8,
}
impl Match {
    pub(crate) fn pattern(&self) -> PatternID {
        self.pid
    }
    pub(crate) fn start(&self) -> *const u8 {
        self.start
    }
    pub(crate) fn end(&self) -> *const u8 {
        self.end
    }
}
#[derive(Clone, Debug)]
pub(crate) struct Slim<V, const BYTES: usize> {
    teddy: Teddy<8>,
    masks: [Mask<V>; BYTES],
}
impl<V: Vector, const BYTES: usize> Slim<V, BYTES> {
    #[inline(always)]
    pub(crate) unsafe fn new(patterns: Arc<Patterns>) -> Slim<V, BYTES> {
        unsafe {
            assert!(
                1 <= BYTES && BYTES <= 4,
                "only 1, 2, 3 or 4 bytes are supported"
            );
            let teddy = Teddy::new(patterns);
            let masks = SlimMaskBuilder::from_teddy(&teddy);
            Slim { teddy, masks }
        }
    }
    #[inline(always)]
    pub(crate) fn memory_usage(&self) -> usize {
        self.teddy.memory_usage()
    }
    #[inline(always)]
    pub(crate) fn minimum_len(&self) -> usize {
        V::BYTES + (BYTES - 1)
    }
}
impl<V: Vector> Slim<V, 1> {
    #[inline(always)]
    pub(crate) unsafe fn find(&self, start: *const u8, end: *const u8) -> Option<Match> {
        unsafe {
            let len = end.distance(start);
            debug_assert!(len >= self.minimum_len());
            let mut cur = start;
            while cur <= end.sub(V::BYTES) {
                if let Some(m) = self.find_one(cur, end) {
                    return Some(m);
                }
                cur = cur.add(V::BYTES);
            }
            if cur < end {
                cur = end.sub(V::BYTES);
                if let Some(m) = self.find_one(cur, end) {
                    return Some(m);
                }
            }
            None
        }
    }
    #[inline(always)]
    unsafe fn find_one(&self, cur: *const u8, end: *const u8) -> Option<Match> {
        unsafe {
            let c = self.candidate(cur);
            if !c.is_zero() {
                if let Some(m) = self.teddy.verify(cur, end, c) {
                    return Some(m);
                }
            }
            None
        }
    }
    #[inline(always)]
    unsafe fn candidate(&self, cur: *const u8) -> V {
        unsafe {
            let chunk = V::load_unaligned(cur);
            Mask::members1(chunk, self.masks)
        }
    }
}
impl<V: Vector> Slim<V, 2> {
    #[inline(always)]
    pub(crate) unsafe fn find(&self, start: *const u8, end: *const u8) -> Option<Match> {
        unsafe {
            let len = end.distance(start);
            debug_assert!(len >= self.minimum_len());
            let mut cur = start.add(1);
            let mut prev0 = V::splat(0xFF);
            while cur <= end.sub(V::BYTES) {
                if let Some(m) = self.find_one(cur, end, &mut prev0) {
                    return Some(m);
                }
                cur = cur.add(V::BYTES);
            }
            if cur < end {
                cur = end.sub(V::BYTES);
                prev0 = V::splat(0xFF);
                if let Some(m) = self.find_one(cur, end, &mut prev0) {
                    return Some(m);
                }
            }
            None
        }
    }
    #[inline(always)]
    unsafe fn find_one(&self, cur: *const u8, end: *const u8, prev0: &mut V) -> Option<Match> {
        unsafe {
            let c = self.candidate(cur, prev0);
            if !c.is_zero() {
                if let Some(m) = self.teddy.verify(cur.sub(1), end, c) {
                    return Some(m);
                }
            }
            None
        }
    }
    #[inline(always)]
    unsafe fn candidate(&self, cur: *const u8, prev0: &mut V) -> V {
        unsafe {
            let chunk = V::load_unaligned(cur);
            let (res0, res1) = Mask::members2(chunk, self.masks);
            let res0prev0 = res0.shift_in_one_byte(*prev0);
            let res = res0prev0.and(res1);
            *prev0 = res0;
            res
        }
    }
}
impl<V: Vector> Slim<V, 3> {
    #[inline(always)]
    pub(crate) unsafe fn find(&self, start: *const u8, end: *const u8) -> Option<Match> {
        unsafe {
            let len = end.distance(start);
            debug_assert!(len >= self.minimum_len());
            let mut cur = start.add(2);
            let mut prev0 = V::splat(0xFF);
            let mut prev1 = V::splat(0xFF);
            while cur <= end.sub(V::BYTES) {
                if let Some(m) = self.find_one(cur, end, &mut prev0, &mut prev1) {
                    return Some(m);
                }
                cur = cur.add(V::BYTES);
            }
            if cur < end {
                cur = end.sub(V::BYTES);
                prev0 = V::splat(0xFF);
                prev1 = V::splat(0xFF);
                if let Some(m) = self.find_one(cur, end, &mut prev0, &mut prev1) {
                    return Some(m);
                }
            }
            None
        }
    }
    #[inline(always)]
    unsafe fn find_one(
        &self,
        cur: *const u8,
        end: *const u8,
        prev0: &mut V,
        prev1: &mut V,
    ) -> Option<Match> {
        unsafe {
            let c = self.candidate(cur, prev0, prev1);
            if !c.is_zero() {
                if let Some(m) = self.teddy.verify(cur.sub(2), end, c) {
                    return Some(m);
                }
            }
            None
        }
    }
    #[inline(always)]
    unsafe fn candidate(&self, cur: *const u8, prev0: &mut V, prev1: &mut V) -> V {
        unsafe {
            let chunk = V::load_unaligned(cur);
            let (res0, res1, res2) = Mask::members3(chunk, self.masks);
            let res0prev0 = res0.shift_in_two_bytes(*prev0);
            let res1prev1 = res1.shift_in_one_byte(*prev1);
            let res = res0prev0.and(res1prev1).and(res2);
            *prev0 = res0;
            *prev1 = res1;
            res
        }
    }
}
impl<V: Vector> Slim<V, 4> {
    #[inline(always)]
    pub(crate) unsafe fn find(&self, start: *const u8, end: *const u8) -> Option<Match> {
        unsafe {
            let len = end.distance(start);
            debug_assert!(len >= self.minimum_len());
            let mut cur = start.add(3);
            let mut prev0 = V::splat(0xFF);
            let mut prev1 = V::splat(0xFF);
            let mut prev2 = V::splat(0xFF);
            while cur <= end.sub(V::BYTES) {
                if let Some(m) = self.find_one(cur, end, &mut prev0, &mut prev1, &mut prev2) {
                    return Some(m);
                }
                cur = cur.add(V::BYTES);
            }
            if cur < end {
                cur = end.sub(V::BYTES);
                prev0 = V::splat(0xFF);
                prev1 = V::splat(0xFF);
                prev2 = V::splat(0xFF);
                if let Some(m) = self.find_one(cur, end, &mut prev0, &mut prev1, &mut prev2) {
                    return Some(m);
                }
            }
            None
        }
    }
    #[inline(always)]
    unsafe fn find_one(
        &self,
        cur: *const u8,
        end: *const u8,
        prev0: &mut V,
        prev1: &mut V,
        prev2: &mut V,
    ) -> Option<Match> {
        unsafe {
            let c = self.candidate(cur, prev0, prev1, prev2);
            if !c.is_zero() {
                if let Some(m) = self.teddy.verify(cur.sub(3), end, c) {
                    return Some(m);
                }
            }
            None
        }
    }
    #[inline(always)]
    unsafe fn candidate(&self, cur: *const u8, prev0: &mut V, prev1: &mut V, prev2: &mut V) -> V {
        unsafe {
            let chunk = V::load_unaligned(cur);
            let (res0, res1, res2, res3) = Mask::members4(chunk, self.masks);
            let res0prev0 = res0.shift_in_three_bytes(*prev0);
            let res1prev1 = res1.shift_in_two_bytes(*prev1);
            let res2prev2 = res2.shift_in_one_byte(*prev2);
            let res = res0prev0.and(res1prev1).and(res2prev2).and(res3);
            *prev0 = res0;
            *prev1 = res1;
            *prev2 = res2;
            res
        }
    }
}
#[derive(Clone, Debug)]
pub(crate) struct Fat<V, const BYTES: usize> {
    teddy: Teddy<16>,
    masks: [Mask<V>; BYTES],
}
impl<V: FatVector, const BYTES: usize> Fat<V, BYTES> {
    #[inline(always)]
    pub(crate) unsafe fn new(patterns: Arc<Patterns>) -> Fat<V, BYTES> {
        unsafe {
            assert!(
                1 <= BYTES && BYTES <= 4,
                "only 1, 2, 3 or 4 bytes are supported"
            );
            let teddy = Teddy::new(patterns);
            let masks = FatMaskBuilder::from_teddy(&teddy);
            Fat { teddy, masks }
        }
    }
    #[inline(always)]
    pub(crate) fn memory_usage(&self) -> usize {
        self.teddy.memory_usage()
    }
    #[inline(always)]
    pub(crate) fn minimum_len(&self) -> usize {
        V::Half::BYTES + (BYTES - 1)
    }
}
impl<V: FatVector> Fat<V, 1> {
    #[inline(always)]
    pub(crate) unsafe fn find(&self, start: *const u8, end: *const u8) -> Option<Match> {
        unsafe {
            let len = end.distance(start);
            debug_assert!(len >= self.minimum_len());
            let mut cur = start;
            while cur <= end.sub(V::Half::BYTES) {
                if let Some(m) = self.find_one(cur, end) {
                    return Some(m);
                }
                cur = cur.add(V::Half::BYTES);
            }
            if cur < end {
                cur = end.sub(V::Half::BYTES);
                if let Some(m) = self.find_one(cur, end) {
                    return Some(m);
                }
            }
            None
        }
    }
    #[inline(always)]
    unsafe fn find_one(&self, cur: *const u8, end: *const u8) -> Option<Match> {
        unsafe {
            let c = self.candidate(cur);
            if !c.is_zero() {
                if let Some(m) = self.teddy.verify(cur, end, c) {
                    return Some(m);
                }
            }
            None
        }
    }
    #[inline(always)]
    unsafe fn candidate(&self, cur: *const u8) -> V {
        unsafe {
            let chunk = V::load_half_unaligned(cur);
            Mask::members1(chunk, self.masks)
        }
    }
}
impl<V: FatVector> Fat<V, 2> {
    #[inline(always)]
    pub(crate) unsafe fn find(&self, start: *const u8, end: *const u8) -> Option<Match> {
        unsafe {
            let len = end.distance(start);
            debug_assert!(len >= self.minimum_len());
            let mut cur = start.add(1);
            let mut prev0 = V::splat(0xFF);
            while cur <= end.sub(V::Half::BYTES) {
                if let Some(m) = self.find_one(cur, end, &mut prev0) {
                    return Some(m);
                }
                cur = cur.add(V::Half::BYTES);
            }
            if cur < end {
                cur = end.sub(V::Half::BYTES);
                prev0 = V::splat(0xFF);
                if let Some(m) = self.find_one(cur, end, &mut prev0) {
                    return Some(m);
                }
            }
            None
        }
    }
    #[inline(always)]
    unsafe fn find_one(&self, cur: *const u8, end: *const u8, prev0: &mut V) -> Option<Match> {
        unsafe {
            let c = self.candidate(cur, prev0);
            if !c.is_zero() {
                if let Some(m) = self.teddy.verify(cur.sub(1), end, c) {
                    return Some(m);
                }
            }
            None
        }
    }
    #[inline(always)]
    unsafe fn candidate(&self, cur: *const u8, prev0: &mut V) -> V {
        unsafe {
            let chunk = V::load_half_unaligned(cur);
            let (res0, res1) = Mask::members2(chunk, self.masks);
            let res0prev0 = res0.half_shift_in_one_byte(*prev0);
            let res = res0prev0.and(res1);
            *prev0 = res0;
            res
        }
    }
}
impl<V: FatVector> Fat<V, 3> {
    #[inline(always)]
    pub(crate) unsafe fn find(&self, start: *const u8, end: *const u8) -> Option<Match> {
        unsafe {
            let len = end.distance(start);
            debug_assert!(len >= self.minimum_len());
            let mut cur = start.add(2);
            let mut prev0 = V::splat(0xFF);
            let mut prev1 = V::splat(0xFF);
            while cur <= end.sub(V::Half::BYTES) {
                if let Some(m) = self.find_one(cur, end, &mut prev0, &mut prev1) {
                    return Some(m);
                }
                cur = cur.add(V::Half::BYTES);
            }
            if cur < end {
                cur = end.sub(V::Half::BYTES);
                prev0 = V::splat(0xFF);
                prev1 = V::splat(0xFF);
                if let Some(m) = self.find_one(cur, end, &mut prev0, &mut prev1) {
                    return Some(m);
                }
            }
            None
        }
    }
    #[inline(always)]
    unsafe fn find_one(
        &self,
        cur: *const u8,
        end: *const u8,
        prev0: &mut V,
        prev1: &mut V,
    ) -> Option<Match> {
        unsafe {
            let c = self.candidate(cur, prev0, prev1);
            if !c.is_zero() {
                if let Some(m) = self.teddy.verify(cur.sub(2), end, c) {
                    return Some(m);
                }
            }
            None
        }
    }
    #[inline(always)]
    unsafe fn candidate(&self, cur: *const u8, prev0: &mut V, prev1: &mut V) -> V {
        unsafe {
            let chunk = V::load_half_unaligned(cur);
            let (res0, res1, res2) = Mask::members3(chunk, self.masks);
            let res0prev0 = res0.half_shift_in_two_bytes(*prev0);
            let res1prev1 = res1.half_shift_in_one_byte(*prev1);
            let res = res0prev0.and(res1prev1).and(res2);
            *prev0 = res0;
            *prev1 = res1;
            res
        }
    }
}
impl<V: FatVector> Fat<V, 4> {
    #[inline(always)]
    pub(crate) unsafe fn find(&self, start: *const u8, end: *const u8) -> Option<Match> {
        unsafe {
            let len = end.distance(start);
            debug_assert!(len >= self.minimum_len());
            let mut cur = start.add(3);
            let mut prev0 = V::splat(0xFF);
            let mut prev1 = V::splat(0xFF);
            let mut prev2 = V::splat(0xFF);
            while cur <= end.sub(V::Half::BYTES) {
                if let Some(m) = self.find_one(cur, end, &mut prev0, &mut prev1, &mut prev2) {
                    return Some(m);
                }
                cur = cur.add(V::Half::BYTES);
            }
            if cur < end {
                cur = end.sub(V::Half::BYTES);
                prev0 = V::splat(0xFF);
                prev1 = V::splat(0xFF);
                prev2 = V::splat(0xFF);
                if let Some(m) = self.find_one(cur, end, &mut prev0, &mut prev1, &mut prev2) {
                    return Some(m);
                }
            }
            None
        }
    }
    #[inline(always)]
    unsafe fn find_one(
        &self,
        cur: *const u8,
        end: *const u8,
        prev0: &mut V,
        prev1: &mut V,
        prev2: &mut V,
    ) -> Option<Match> {
        unsafe {
            let c = self.candidate(cur, prev0, prev1, prev2);
            if !c.is_zero() {
                if let Some(m) = self.teddy.verify(cur.sub(3), end, c) {
                    return Some(m);
                }
            }
            None
        }
    }
    #[inline(always)]
    unsafe fn candidate(&self, cur: *const u8, prev0: &mut V, prev1: &mut V, prev2: &mut V) -> V {
        unsafe {
            let chunk = V::load_half_unaligned(cur);
            let (res0, res1, res2, res3) = Mask::members4(chunk, self.masks);
            let res0prev0 = res0.half_shift_in_three_bytes(*prev0);
            let res1prev1 = res1.half_shift_in_two_bytes(*prev1);
            let res2prev2 = res2.half_shift_in_one_byte(*prev2);
            let res = res0prev0.and(res1prev1).and(res2prev2).and(res3);
            *prev0 = res0;
            *prev1 = res1;
            *prev2 = res2;
            res
        }
    }
}
#[derive(Clone, Debug)]
struct Teddy<const BUCKETS: usize> {
    patterns: Arc<Patterns>,
    buckets: [Vec<PatternID>; BUCKETS],
}
impl<const BUCKETS: usize> Teddy<BUCKETS> {
    fn new(patterns: Arc<Patterns>) -> Teddy<BUCKETS> {
        assert_ne!(0, patterns.len(), "Teddy requires at least one pattern");
        assert_ne!(
            0,
            patterns.minimum_len(),
            "Teddy does not support zero-length patterns"
        );
        assert!(
            BUCKETS == 8 || BUCKETS == 16,
            "Teddy only supports 8 or 16 buckets"
        );
        let buckets = <[Vec<PatternID>; BUCKETS]>::try_from(vec![vec![]; BUCKETS]).unwrap();
        let mut t = Teddy { patterns, buckets };
        let mut map: BTreeMap<Box<[u8]>, usize> = BTreeMap::new();
        for (id, pattern) in t.patterns.iter() {
            let lonybs = pattern.low_nybbles(t.mask_len());
            if let Some(&bucket) = map.get(&lonybs) {
                t.buckets[bucket].push(id);
            } else {
                let bucket = (BUCKETS - 1) - (id.as_usize() % BUCKETS);
                t.buckets[bucket].push(id);
                map.insert(lonybs, bucket);
            }
        }
        t
    }
    #[inline(always)]
    unsafe fn verify64(
        &self,
        cur: *const u8,
        end: *const u8,
        mut candidate_chunk: u64,
    ) -> Option<Match> {
        unsafe {
            while candidate_chunk != 0 {
                let bit = candidate_chunk.trailing_zeros().as_usize();
                candidate_chunk &= !(1 << bit);
                let cur = cur.add(bit / BUCKETS);
                let bucket = bit % BUCKETS;
                if let Some(m) = self.verify_bucket(cur, end, bucket) {
                    return Some(m);
                }
            }
            None
        }
    }
    #[inline(always)]
    unsafe fn verify_bucket(&self, cur: *const u8, end: *const u8, bucket: usize) -> Option<Match> {
        unsafe {
            debug_assert!(bucket < self.buckets.len());
            for pid in self.buckets.get_unchecked(bucket).iter().copied() {
                debug_assert!(pid.as_usize() < self.patterns.len());
                let pat = self.patterns.get_unchecked(pid);
                if pat.is_prefix_raw(cur, end) {
                    let start = cur;
                    let end = start.add(pat.len());
                    return Some(Match { pid, start, end });
                }
            }
            None
        }
    }
    fn mask_len(&self) -> usize {
        core::cmp::min(4, self.patterns.minimum_len())
    }
    fn memory_usage(&self) -> usize {
        self.patterns.len() * core::mem::size_of::<PatternID>()
    }
}
impl Teddy<8> {
    #[inline(always)]
    unsafe fn verify<V: Vector>(
        &self,
        mut cur: *const u8,
        end: *const u8,
        candidate: V,
    ) -> Option<Match> {
        unsafe {
            debug_assert!(!candidate.is_zero());
            candidate.for_each_64bit_lane(
                #[inline(always)]
                |_, chunk| {
                    let result = self.verify64(cur, end, chunk);
                    cur = cur.add(8);
                    result
                },
            )
        }
    }
}
impl Teddy<16> {
    #[inline(always)]
    unsafe fn verify<V: FatVector>(
        &self,
        mut cur: *const u8,
        end: *const u8,
        candidate: V,
    ) -> Option<Match> {
        unsafe {
            debug_assert!(!candidate.is_zero());
            let swapped = candidate.swap_halves();
            let r1 = candidate.interleave_low_8bit_lanes(swapped);
            let r2 = candidate.interleave_high_8bit_lanes(swapped);
            r1.for_each_low_64bit_lane(
                r2,
                #[inline(always)]
                |_, chunk| {
                    let result = self.verify64(cur, end, chunk);
                    cur = cur.add(4);
                    result
                },
            )
        }
    }
}
#[derive(Clone, Copy, Debug)]
struct Mask<V> {
    lo: V,
    hi: V,
}
impl<V: Vector> Mask<V> {
    #[inline(always)]
    unsafe fn members1(chunk: V, masks: [Mask<V>; 1]) -> V {
        unsafe {
            let lomask = V::splat(0xF);
            let hlo = chunk.and(lomask);
            let hhi = chunk.shift_8bit_lane_right::<4>().and(lomask);
            let locand = masks[0].lo.shuffle_bytes(hlo);
            let hicand = masks[0].hi.shuffle_bytes(hhi);
            locand.and(hicand)
        }
    }
    #[inline(always)]
    unsafe fn members2(chunk: V, masks: [Mask<V>; 2]) -> (V, V) {
        unsafe {
            let lomask = V::splat(0xF);
            let hlo = chunk.and(lomask);
            let hhi = chunk.shift_8bit_lane_right::<4>().and(lomask);
            let locand1 = masks[0].lo.shuffle_bytes(hlo);
            let hicand1 = masks[0].hi.shuffle_bytes(hhi);
            let cand1 = locand1.and(hicand1);
            let locand2 = masks[1].lo.shuffle_bytes(hlo);
            let hicand2 = masks[1].hi.shuffle_bytes(hhi);
            let cand2 = locand2.and(hicand2);
            (cand1, cand2)
        }
    }
    #[inline(always)]
    unsafe fn members3(chunk: V, masks: [Mask<V>; 3]) -> (V, V, V) {
        unsafe {
            let lomask = V::splat(0xF);
            let hlo = chunk.and(lomask);
            let hhi = chunk.shift_8bit_lane_right::<4>().and(lomask);
            let locand1 = masks[0].lo.shuffle_bytes(hlo);
            let hicand1 = masks[0].hi.shuffle_bytes(hhi);
            let cand1 = locand1.and(hicand1);
            let locand2 = masks[1].lo.shuffle_bytes(hlo);
            let hicand2 = masks[1].hi.shuffle_bytes(hhi);
            let cand2 = locand2.and(hicand2);
            let locand3 = masks[2].lo.shuffle_bytes(hlo);
            let hicand3 = masks[2].hi.shuffle_bytes(hhi);
            let cand3 = locand3.and(hicand3);
            (cand1, cand2, cand3)
        }
    }
    #[inline(always)]
    unsafe fn members4(chunk: V, masks: [Mask<V>; 4]) -> (V, V, V, V) {
        unsafe {
            let lomask = V::splat(0xF);
            let hlo = chunk.and(lomask);
            let hhi = chunk.shift_8bit_lane_right::<4>().and(lomask);
            let locand1 = masks[0].lo.shuffle_bytes(hlo);
            let hicand1 = masks[0].hi.shuffle_bytes(hhi);
            let cand1 = locand1.and(hicand1);
            let locand2 = masks[1].lo.shuffle_bytes(hlo);
            let hicand2 = masks[1].hi.shuffle_bytes(hhi);
            let cand2 = locand2.and(hicand2);
            let locand3 = masks[2].lo.shuffle_bytes(hlo);
            let hicand3 = masks[2].hi.shuffle_bytes(hhi);
            let cand3 = locand3.and(hicand3);
            let locand4 = masks[3].lo.shuffle_bytes(hlo);
            let hicand4 = masks[3].hi.shuffle_bytes(hhi);
            let cand4 = locand4.and(hicand4);
            (cand1, cand2, cand3, cand4)
        }
    }
}
#[derive(Clone, Default)]
struct SlimMaskBuilder {
    lo: [u8; 32],
    hi: [u8; 32],
}
impl SlimMaskBuilder {
    fn add(&mut self, bucket: usize, byte: u8) {
        assert!(bucket < 8);
        let bucket = u8::try_from(bucket).unwrap();
        let byte_lo = usize::from(byte & 0xF);
        let byte_hi = usize::from((byte >> 4) & 0xF);
        self.lo[byte_lo] |= 1 << bucket;
        self.lo[byte_lo + 16] |= 1 << bucket;
        self.hi[byte_hi] |= 1 << bucket;
        self.hi[byte_hi + 16] |= 1 << bucket;
    }
    #[inline(always)]
    unsafe fn build<V: Vector>(&self) -> Mask<V> {
        unsafe {
            assert!(V::BYTES <= self.lo.len());
            assert!(V::BYTES <= self.hi.len());
            Mask {
                lo: V::load_unaligned(self.lo[..].as_ptr()),
                hi: V::load_unaligned(self.hi[..].as_ptr()),
            }
        }
    }
    #[inline(always)]
    unsafe fn from_teddy<const BYTES: usize, V: Vector>(teddy: &Teddy<8>) -> [Mask<V>; BYTES] {
        unsafe {
            let mut mask_builders = vec![SlimMaskBuilder::default(); BYTES];
            for (bucket_index, bucket) in teddy.buckets.iter().enumerate() {
                for pid in bucket.iter().copied() {
                    let pat = teddy.patterns.get(pid);
                    for (i, builder) in mask_builders.iter_mut().enumerate() {
                        builder.add(bucket_index, pat.bytes()[i]);
                    }
                }
            }
            let array = <[SlimMaskBuilder; BYTES]>::try_from(mask_builders).unwrap();
            array.map(|builder| builder.build())
        }
    }
}
impl Debug for SlimMaskBuilder {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let (mut parts_lo, mut parts_hi) = (vec![], vec![]);
        for i in 0..32 {
            parts_lo.push(format!("{:02}: {:08b}", i, self.lo[i]));
            parts_hi.push(format!("{:02}: {:08b}", i, self.hi[i]));
        }
        f.debug_struct("SlimMaskBuilder")
            .field("lo", &parts_lo)
            .field("hi", &parts_hi)
            .finish()
    }
}
#[derive(Clone, Copy, Default)]
struct FatMaskBuilder {
    lo: [u8; 32],
    hi: [u8; 32],
}
impl FatMaskBuilder {
    fn add(&mut self, bucket: usize, byte: u8) {
        assert!(bucket < 16);
        let bucket = u8::try_from(bucket).unwrap();
        let byte_lo = usize::from(byte & 0xF);
        let byte_hi = usize::from((byte >> 4) & 0xF);
        if bucket < 8 {
            self.lo[byte_lo] |= 1 << bucket;
            self.hi[byte_hi] |= 1 << bucket;
        } else {
            self.lo[byte_lo + 16] |= 1 << (bucket % 8);
            self.hi[byte_hi + 16] |= 1 << (bucket % 8);
        }
    }
    #[inline(always)]
    unsafe fn build<V: Vector>(&self) -> Mask<V> {
        unsafe {
            assert!(V::BYTES <= self.lo.len());
            assert!(V::BYTES <= self.hi.len());
            Mask {
                lo: V::load_unaligned(self.lo[..].as_ptr()),
                hi: V::load_unaligned(self.hi[..].as_ptr()),
            }
        }
    }
    #[inline(always)]
    unsafe fn from_teddy<const BYTES: usize, V: Vector>(teddy: &Teddy<16>) -> [Mask<V>; BYTES] {
        unsafe {
            let mut mask_builders = vec![FatMaskBuilder::default(); BYTES];
            for (bucket_index, bucket) in teddy.buckets.iter().enumerate() {
                for pid in bucket.iter().copied() {
                    let pat = teddy.patterns.get(pid);
                    for (i, builder) in mask_builders.iter_mut().enumerate() {
                        builder.add(bucket_index, pat.bytes()[i]);
                    }
                }
            }
            let array = <[FatMaskBuilder; BYTES]>::try_from(mask_builders).unwrap();
            array.map(|builder| builder.build())
        }
    }
}
impl Debug for FatMaskBuilder {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let (mut parts_lo, mut parts_hi) = (vec![], vec![]);
        for i in 0..32 {
            parts_lo.push(format!("{:02}: {:08b}", i, self.lo[i]));
            parts_hi.push(format!("{:02}: {:08b}", i, self.hi[i]));
        }
        f.debug_struct("FatMaskBuilder")
            .field("lo", &parts_lo)
            .field("hi", &parts_hi)
            .finish()
    }
}
