use crate::memchr::{
    arch::all::{is_prefix, is_suffix},
    memmem::Pre,
};
use core::cmp;
#[derive(Clone, Copy, Debug)]
pub struct Finder(TwoWay);
#[derive(Clone, Copy, Debug)]
pub struct FinderRev(TwoWay);
#[derive(Clone, Copy, Debug)]
struct TwoWay {
    byteset: ApproximateByteSet,
    critical_pos: usize,
    shift: Shift,
}
impl Finder {
    #[inline]
    pub fn new(needle: &[u8]) -> Finder {
        let byteset = ApproximateByteSet::new(needle);
        let min_suffix = Suffix::forward(needle, SuffixKind::Minimal);
        let max_suffix = Suffix::forward(needle, SuffixKind::Maximal);
        let (period_lower_bound, critical_pos) = if min_suffix.pos > max_suffix.pos {
            (min_suffix.period, min_suffix.pos)
        } else {
            (max_suffix.period, max_suffix.pos)
        };
        let shift = Shift::forward(needle, period_lower_bound, critical_pos);
        Finder(TwoWay {
            byteset,
            critical_pos,
            shift,
        })
    }
    #[inline]
    pub fn find(&self, haystack: &[u8], needle: &[u8]) -> Option<usize> {
        self.find_with_prefilter(None, haystack, needle)
    }
    #[inline(always)]
    pub(crate) fn find_with_prefilter(
        &self,
        pre: Option<Pre<'_>>,
        haystack: &[u8],
        needle: &[u8],
    ) -> Option<usize> {
        match self.0.shift {
            Shift::Small { period } => self.find_small_imp(pre, haystack, needle, period),
            Shift::Large { shift } => self.find_large_imp(pre, haystack, needle, shift),
        }
    }
    #[inline(always)]
    fn find_small_imp(
        &self,
        mut pre: Option<Pre<'_>>,
        haystack: &[u8],
        needle: &[u8],
        period: usize,
    ) -> Option<usize> {
        let mut pos = 0;
        let mut shift = 0;
        let last_byte_pos = match needle.len().checked_sub(1) {
            None => return Some(pos),
            Some(last_byte) => last_byte,
        };
        while pos + needle.len() <= haystack.len() {
            let mut i = cmp::max(self.0.critical_pos, shift);
            if let Some(pre) = pre.as_mut() {
                if pre.is_effective() {
                    pos += pre.find(&haystack[pos..])?;
                    shift = 0;
                    i = self.0.critical_pos;
                    if pos + needle.len() > haystack.len() {
                        return None;
                    }
                }
            }
            if !self.0.byteset.contains(haystack[pos + last_byte_pos]) {
                pos += needle.len();
                shift = 0;
                continue;
            }
            while i < needle.len() && needle[i] == haystack[pos + i] {
                i += 1;
            }
            if i < needle.len() {
                pos += i - self.0.critical_pos + 1;
                shift = 0;
            } else {
                let mut j = self.0.critical_pos;
                while j > shift && needle[j] == haystack[pos + j] {
                    j -= 1;
                }
                if j <= shift && needle[shift] == haystack[pos + shift] {
                    return Some(pos);
                }
                pos += period;
                shift = needle.len() - period;
            }
        }
        None
    }
    #[inline(always)]
    fn find_large_imp(
        &self,
        mut pre: Option<Pre<'_>>,
        haystack: &[u8],
        needle: &[u8],
        shift: usize,
    ) -> Option<usize> {
        let mut pos = 0;
        let last_byte_pos = match needle.len().checked_sub(1) {
            None => return Some(pos),
            Some(last_byte) => last_byte,
        };
        'outer: while pos + needle.len() <= haystack.len() {
            if let Some(pre) = pre.as_mut() {
                if pre.is_effective() {
                    pos += pre.find(&haystack[pos..])?;
                    if pos + needle.len() > haystack.len() {
                        return None;
                    }
                }
            }
            if !self.0.byteset.contains(haystack[pos + last_byte_pos]) {
                pos += needle.len();
                continue;
            }
            let mut i = self.0.critical_pos;
            while i < needle.len() && needle[i] == haystack[pos + i] {
                i += 1;
            }
            if i < needle.len() {
                pos += i - self.0.critical_pos + 1;
            } else {
                for j in (0..self.0.critical_pos).rev() {
                    if needle[j] != haystack[pos + j] {
                        pos += shift;
                        continue 'outer;
                    }
                }
                return Some(pos);
            }
        }
        None
    }
}
impl FinderRev {
    #[inline]
    pub fn new(needle: &[u8]) -> FinderRev {
        let byteset = ApproximateByteSet::new(needle);
        let min_suffix = Suffix::reverse(needle, SuffixKind::Minimal);
        let max_suffix = Suffix::reverse(needle, SuffixKind::Maximal);
        let (period_lower_bound, critical_pos) = if min_suffix.pos < max_suffix.pos {
            (min_suffix.period, min_suffix.pos)
        } else {
            (max_suffix.period, max_suffix.pos)
        };
        let shift = Shift::reverse(needle, period_lower_bound, critical_pos);
        FinderRev(TwoWay {
            byteset,
            critical_pos,
            shift,
        })
    }
    #[inline]
    pub fn rfind(&self, haystack: &[u8], needle: &[u8]) -> Option<usize> {
        match self.0.shift {
            Shift::Small { period } => self.rfind_small_imp(haystack, needle, period),
            Shift::Large { shift } => self.rfind_large_imp(haystack, needle, shift),
        }
    }
    #[inline(always)]
    fn rfind_small_imp(&self, haystack: &[u8], needle: &[u8], period: usize) -> Option<usize> {
        let nlen = needle.len();
        let mut pos = haystack.len();
        let mut shift = nlen;
        let first_byte = match needle.first() {
            None => return Some(pos),
            Some(&first_byte) => first_byte,
        };
        while pos >= nlen {
            if !self.0.byteset.contains(haystack[pos - nlen]) {
                pos -= nlen;
                shift = nlen;
                continue;
            }
            let mut i = cmp::min(self.0.critical_pos, shift);
            while i > 0 && needle[i - 1] == haystack[pos - nlen + i - 1] {
                i -= 1;
            }
            if i > 0 || first_byte != haystack[pos - nlen] {
                pos -= self.0.critical_pos - i + 1;
                shift = nlen;
            } else {
                let mut j = self.0.critical_pos;
                while j < shift && needle[j] == haystack[pos - nlen + j] {
                    j += 1;
                }
                if j >= shift {
                    return Some(pos - nlen);
                }
                pos -= period;
                shift = period;
            }
        }
        None
    }
    #[inline(always)]
    fn rfind_large_imp(&self, haystack: &[u8], needle: &[u8], shift: usize) -> Option<usize> {
        let nlen = needle.len();
        let mut pos = haystack.len();
        let first_byte = match needle.first() {
            None => return Some(pos),
            Some(&first_byte) => first_byte,
        };
        while pos >= nlen {
            if !self.0.byteset.contains(haystack[pos - nlen]) {
                pos -= nlen;
                continue;
            }
            let mut i = self.0.critical_pos;
            while i > 0 && needle[i - 1] == haystack[pos - nlen + i - 1] {
                i -= 1;
            }
            if i > 0 || first_byte != haystack[pos - nlen] {
                pos -= self.0.critical_pos - i + 1;
            } else {
                let mut j = self.0.critical_pos;
                while j < nlen && needle[j] == haystack[pos - nlen + j] {
                    j += 1;
                }
                if j == nlen {
                    return Some(pos - nlen);
                }
                pos -= shift;
            }
        }
        None
    }
}
#[derive(Clone, Copy, Debug)]
enum Shift {
    Small { period: usize },
    Large { shift: usize },
}
impl Shift {
    fn forward(needle: &[u8], period_lower_bound: usize, critical_pos: usize) -> Shift {
        let large = cmp::max(critical_pos, needle.len() - critical_pos);
        if critical_pos * 2 >= needle.len() {
            return Shift::Large { shift: large };
        }
        let (u, v) = needle.split_at(critical_pos);
        if !is_suffix(&v[..period_lower_bound], u) {
            return Shift::Large { shift: large };
        }
        Shift::Small {
            period: period_lower_bound,
        }
    }
    fn reverse(needle: &[u8], period_lower_bound: usize, critical_pos: usize) -> Shift {
        let large = cmp::max(critical_pos, needle.len() - critical_pos);
        if (needle.len() - critical_pos) * 2 >= needle.len() {
            return Shift::Large { shift: large };
        }
        let (v, u) = needle.split_at(critical_pos);
        if !is_prefix(&v[v.len() - period_lower_bound..], u) {
            return Shift::Large { shift: large };
        }
        Shift::Small {
            period: period_lower_bound,
        }
    }
}
#[derive(Debug)]
struct Suffix {
    pos: usize,
    period: usize,
}
impl Suffix {
    fn forward(needle: &[u8], kind: SuffixKind) -> Suffix {
        let mut suffix = Suffix { pos: 0, period: 1 };
        let mut candidate_start = 1;
        let mut offset = 0;
        while candidate_start + offset < needle.len() {
            let current = needle[suffix.pos + offset];
            let candidate = needle[candidate_start + offset];
            match kind.cmp(current, candidate) {
                SuffixOrdering::Accept => {
                    suffix = Suffix {
                        pos: candidate_start,
                        period: 1,
                    };
                    candidate_start += 1;
                    offset = 0;
                }
                SuffixOrdering::Skip => {
                    candidate_start += offset + 1;
                    offset = 0;
                    suffix.period = candidate_start - suffix.pos;
                }
                SuffixOrdering::Push => {
                    if offset + 1 == suffix.period {
                        candidate_start += suffix.period;
                        offset = 0;
                    } else {
                        offset += 1;
                    }
                }
            }
        }
        suffix
    }
    fn reverse(needle: &[u8], kind: SuffixKind) -> Suffix {
        let mut suffix = Suffix {
            pos: needle.len(),
            period: 1,
        };
        if needle.len() == 1 {
            return suffix;
        }
        let mut candidate_start = match needle.len().checked_sub(1) {
            None => return suffix,
            Some(candidate_start) => candidate_start,
        };
        let mut offset = 0;
        while offset < candidate_start {
            let current = needle[suffix.pos - offset - 1];
            let candidate = needle[candidate_start - offset - 1];
            match kind.cmp(current, candidate) {
                SuffixOrdering::Accept => {
                    suffix = Suffix {
                        pos: candidate_start,
                        period: 1,
                    };
                    candidate_start -= 1;
                    offset = 0;
                }
                SuffixOrdering::Skip => {
                    candidate_start -= offset + 1;
                    offset = 0;
                    suffix.period = suffix.pos - candidate_start;
                }
                SuffixOrdering::Push => {
                    if offset + 1 == suffix.period {
                        candidate_start -= suffix.period;
                        offset = 0;
                    } else {
                        offset += 1;
                    }
                }
            }
        }
        suffix
    }
}
#[derive(Clone, Copy, Debug)]
enum SuffixKind {
    Minimal,
    Maximal,
}
#[derive(Clone, Copy, Debug)]
enum SuffixOrdering {
    Accept,
    Skip,
    Push,
}
impl SuffixKind {
    fn cmp(self, current: u8, candidate: u8) -> SuffixOrdering {
        use self::SuffixOrdering::*;
        match self {
            SuffixKind::Minimal if candidate < current => Accept,
            SuffixKind::Minimal if candidate > current => Skip,
            SuffixKind::Minimal => Push,
            SuffixKind::Maximal if candidate > current => Accept,
            SuffixKind::Maximal if candidate < current => Skip,
            SuffixKind::Maximal => Push,
        }
    }
}
#[derive(Clone, Copy, Debug)]
struct ApproximateByteSet(u64);
impl ApproximateByteSet {
    fn new(needle: &[u8]) -> ApproximateByteSet {
        let mut bits = 0;
        for &b in needle {
            bits |= 1 << (b % 64);
        }
        ApproximateByteSet(bits)
    }
    #[inline(always)]
    fn contains(&self, byte: u8) -> bool {
        self.0 & (1 << (byte % 64)) != 0
    }
}
