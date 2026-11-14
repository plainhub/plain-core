use crate::memchr::ext::Pointer;
use crate::regex_engine::aho::{PatternID, packed::api::MatchKind};
use core::{cmp, fmt, mem, u16, usize};
use std::{boxed::Box, string::String, vec, vec::Vec};
#[derive(Clone, Debug)]
pub(crate) struct Patterns {
    kind: MatchKind,
    by_id: Vec<Vec<u8>>,
    order: Vec<PatternID>,
    minimum_len: usize,
    total_pattern_bytes: usize,
}
impl Patterns {
    pub(crate) fn new() -> Patterns {
        Patterns {
            kind: MatchKind::default(),
            by_id: vec![],
            order: vec![],
            minimum_len: usize::MAX,
            total_pattern_bytes: 0,
        }
    }
    pub(crate) fn add(&mut self, bytes: &[u8]) {
        assert!(!bytes.is_empty());
        assert!(self.by_id.len() <= u16::MAX as usize);
        let id = PatternID::new(self.by_id.len()).unwrap();
        self.order.push(id);
        self.by_id.push(bytes.to_vec());
        self.minimum_len = cmp::min(self.minimum_len, bytes.len());
        self.total_pattern_bytes += bytes.len();
    }
    pub(crate) fn set_match_kind(&mut self, kind: MatchKind) {
        self.kind = kind;
        match self.kind {
            MatchKind::LeftmostFirst => {
                self.order.sort();
            }
            MatchKind::LeftmostLongest => {
                let (order, by_id) = (&mut self.order, &mut self.by_id);
                order.sort_by(|&id1, &id2| by_id[id1].len().cmp(&by_id[id2].len()).reverse());
            }
        }
    }
    pub(crate) fn len(&self) -> usize {
        self.by_id.len()
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub(crate) fn memory_usage(&self) -> usize {
        self.order.len() * mem::size_of::<PatternID>()
            + self.by_id.len() * mem::size_of::<Vec<u8>>()
            + self.total_pattern_bytes
    }
    pub(crate) fn reset(&mut self) {
        self.kind = MatchKind::default();
        self.by_id.clear();
        self.order.clear();
        self.minimum_len = usize::MAX;
    }
    pub(crate) fn minimum_len(&self) -> usize {
        self.minimum_len
    }
    pub(crate) fn match_kind(&self) -> &MatchKind {
        &self.kind
    }
    pub(crate) fn get(&self, id: PatternID) -> Pattern<'_> {
        Pattern(&self.by_id[id])
    }
    pub(crate) unsafe fn get_unchecked(&self, id: PatternID) -> Pattern<'_> {
        unsafe { Pattern(self.by_id.get_unchecked(id.as_usize())) }
    }
    pub(crate) fn iter(&self) -> PatternIter<'_> {
        PatternIter {
            patterns: self,
            i: 0,
        }
    }
}
#[derive(Debug)]
pub(crate) struct PatternIter<'p> {
    patterns: &'p Patterns,
    i: usize,
}
impl<'p> Iterator for PatternIter<'p> {
    type Item = (PatternID, Pattern<'p>);
    fn next(&mut self) -> Option<(PatternID, Pattern<'p>)> {
        if self.i >= self.patterns.len() {
            return None;
        }
        let id = self.patterns.order[self.i];
        let p = self.patterns.get(id);
        self.i += 1;
        Some((id, p))
    }
}
#[derive(Clone)]
pub(crate) struct Pattern<'a>(&'a [u8]);
impl<'a> fmt::Debug for Pattern<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pattern")
            .field("lit", &String::from_utf8_lossy(self.0))
            .finish()
    }
}
impl<'p> Pattern<'p> {
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }
    pub(crate) fn bytes(&self) -> &[u8] {
        self.0
    }
    pub(crate) fn low_nybbles(&self, len: usize) -> Box<[u8]> {
        let mut nybs = vec![0; len].into_boxed_slice();
        for (i, byte) in self.bytes().iter().take(len).enumerate() {
            nybs[i] = byte & 0xF;
        }
        nybs
    }
    #[inline(always)]
    pub(crate) fn is_prefix(&self, bytes: &[u8]) -> bool {
        is_prefix(bytes, self.bytes())
    }
    #[inline(always)]
    pub(crate) unsafe fn is_prefix_raw(&self, start: *const u8, end: *const u8) -> bool {
        unsafe {
            let patlen = self.bytes().len();
            let haylen = end.distance(start);
            if patlen > haylen {
                return false;
            }
            is_equal_raw(start, self.bytes().as_ptr(), patlen)
        }
    }
}
#[inline(always)]
fn is_prefix(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.len() > haystack.len() {
        return false;
    }
    unsafe { is_equal_raw(haystack.as_ptr(), needle.as_ptr(), needle.len()) }
}
#[inline(always)]
unsafe fn is_equal_raw(mut x: *const u8, mut y: *const u8, n: usize) -> bool {
    unsafe {
        if n < 4 {
            return match n {
                0 => true,
                1 => x.read() == y.read(),
                2 => x.cast::<u16>().read_unaligned() == y.cast::<u16>().read_unaligned(),
                3 => x.cast::<[u8; 3]>().read() == y.cast::<[u8; 3]>().read(),
                _ => unreachable!(),
            };
        }
        let xend = x.add(n.wrapping_sub(4));
        let yend = y.add(n.wrapping_sub(4));
        while x < xend {
            let vx = x.cast::<u32>().read_unaligned();
            let vy = y.cast::<u32>().read_unaligned();
            if vx != vy {
                return false;
            }
            x = x.add(4);
            y = y.add(4);
        }
        let vx = xend.cast::<u32>().read_unaligned();
        let vy = yend.cast::<u32>().read_unaligned();
        vx == vy
    }
}
