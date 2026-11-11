use crate::regex_engine::syntax::unicode;
use core::{char, cmp, fmt::Debug, slice};
use std::vec::Vec;
#[derive(Clone, Debug)]
pub struct IntervalSet<I> {
    ranges: Vec<I>,
    folded: bool,
}
impl<I: Interval> Eq for IntervalSet<I> {}
impl<I: Interval> PartialEq for IntervalSet<I> {
    fn eq(&self, other: &IntervalSet<I>) -> bool {
        self.ranges.eq(&other.ranges)
    }
}
impl<I: Interval> IntervalSet<I> {
    pub fn new<T: IntoIterator<Item = I>>(intervals: T) -> IntervalSet<I> {
        let ranges: Vec<I> = intervals.into_iter().collect();
        let folded = ranges.is_empty();
        let mut set = IntervalSet { ranges, folded };
        set.canonicalize();
        set
    }
    pub fn push(&mut self, mut interval: I) {
        let Err(i) = self.ranges.binary_search(&interval) else {
            return;
        };
        let mut start = i;
        if let Some(before_i) = i.checked_sub(1) {
            let before = &self.ranges[before_i];
            if let Some(union) = before.union(&interval) {
                interval = union;
                start = before_i;
            }
        }
        let mut end = i;
        for after_i in i..self.ranges.len() {
            let after = &self.ranges[after_i];
            let Some(union) = interval.union(after) else {
                break;
            };
            interval = union;
            end = after_i + 1;
        }
        self.ranges.splice(start..end, core::iter::once(interval));
        self.folded = false;
    }
    pub fn iter(&self) -> IntervalSetIter<'_, I> {
        IntervalSetIter(self.ranges.iter())
    }
    pub fn intervals(&self) -> &[I] {
        &self.ranges
    }
    pub fn case_fold_simple(&mut self) -> Result<(), unicode::CaseFoldError> {
        if self.folded {
            return Ok(());
        }
        let len = self.ranges.len();
        for i in 0..len {
            let range = self.ranges[i];
            if let Err(err) = range.case_fold_simple(&mut self.ranges) {
                self.canonicalize();
                return Err(err);
            }
        }
        self.canonicalize();
        self.folded = true;
        Ok(())
    }
    pub fn union(&mut self, other: &IntervalSet<I>) {
        if other.ranges.is_empty() || self.ranges == other.ranges {
            return;
        }
        self.ranges.extend(&other.ranges);
        self.canonicalize();
        self.folded = self.folded && other.folded;
    }
    pub fn intersect(&mut self, other: &IntervalSet<I>) {
        if self.ranges.is_empty() {
            return;
        }
        if other.ranges.is_empty() {
            self.ranges.clear();
            self.folded = true;
            return;
        }
        let drain_end = self.ranges.len();
        let mut ita = 0..drain_end;
        let mut itb = 0..other.ranges.len();
        let mut a = ita.next().unwrap();
        let mut b = itb.next().unwrap();
        loop {
            if let Some(ab) = self.ranges[a].intersect(&other.ranges[b]) {
                self.ranges.push(ab);
            }
            let (it, aorb) = if self.ranges[a].upper() < other.ranges[b].upper() {
                (&mut ita, &mut a)
            } else {
                (&mut itb, &mut b)
            };
            match it.next() {
                Some(v) => *aorb = v,
                None => break,
            }
        }
        self.ranges.drain(..drain_end);
        self.folded = self.folded && other.folded;
    }
    pub fn difference(&mut self, other: &IntervalSet<I>) {
        if self.ranges.is_empty() || other.ranges.is_empty() {
            return;
        }
        let drain_end = self.ranges.len();
        let (mut a, mut b) = (0, 0);
        'LOOP: while a < drain_end && b < other.ranges.len() {
            if other.ranges[b].upper() < self.ranges[a].lower() {
                b += 1;
                continue;
            }
            if self.ranges[a].upper() < other.ranges[b].lower() {
                let range = self.ranges[a];
                self.ranges.push(range);
                a += 1;
                continue;
            }
            assert!(!self.ranges[a].is_intersection_empty(&other.ranges[b]));
            let mut range = self.ranges[a];
            while b < other.ranges.len() && !range.is_intersection_empty(&other.ranges[b]) {
                let old_range = range;
                range = match range.difference(&other.ranges[b]) {
                    (None, None) => {
                        a += 1;
                        continue 'LOOP;
                    }
                    (Some(range1), None) | (None, Some(range1)) => range1,
                    (Some(range1), Some(range2)) => {
                        self.ranges.push(range1);
                        range2
                    }
                };
                if other.ranges[b].upper() > old_range.upper() {
                    break;
                }
                b += 1;
            }
            self.ranges.push(range);
            a += 1;
        }
        while a < drain_end {
            let range = self.ranges[a];
            self.ranges.push(range);
            a += 1;
        }
        self.ranges.drain(..drain_end);
        self.folded = self.folded && other.folded;
    }
    pub fn symmetric_difference(&mut self, other: &IntervalSet<I>) {
        let mut intersection = self.clone();
        intersection.intersect(other);
        self.union(other);
        self.difference(&intersection);
    }
    pub fn negate(&mut self) {
        if self.ranges.is_empty() {
            let (min, max) = (I::Bound::min_value(), I::Bound::max_value());
            self.ranges.push(I::create(min, max));
            self.folded = true;
            return;
        }
        let drain_end = self.ranges.len();
        if self.ranges[0].lower() > I::Bound::min_value() {
            let upper = self.ranges[0].lower().decrement();
            self.ranges.push(I::create(I::Bound::min_value(), upper));
        }
        for i in 1..drain_end {
            let lower = self.ranges[i - 1].upper().increment();
            let upper = self.ranges[i].lower().decrement();
            self.ranges.push(I::create(lower, upper));
        }
        if self.ranges[drain_end - 1].upper() < I::Bound::max_value() {
            let lower = self.ranges[drain_end - 1].upper().increment();
            self.ranges.push(I::create(lower, I::Bound::max_value()));
        }
        self.ranges.drain(..drain_end);
    }
    fn canonicalize(&mut self) {
        if self.is_canonical() {
            return;
        }
        self.ranges.sort();
        assert!(!self.ranges.is_empty());
        let drain_end = self.ranges.len();
        for oldi in 0..drain_end {
            if self.ranges.len() > drain_end {
                let (last, rest) = self.ranges.split_last_mut().unwrap();
                if let Some(union) = last.union(&rest[oldi]) {
                    *last = union;
                    continue;
                }
            }
            let range = self.ranges[oldi];
            self.ranges.push(range);
        }
        self.ranges.drain(..drain_end);
    }
    fn is_canonical(&self) -> bool {
        for pair in self.ranges.windows(2) {
            if pair[0] >= pair[1] {
                return false;
            }
            if pair[0].is_contiguous(&pair[1]) {
                return false;
            }
        }
        true
    }
}
#[derive(Debug)]
pub struct IntervalSetIter<'a, I>(slice::Iter<'a, I>);
impl<'a, I> Iterator for IntervalSetIter<'a, I> {
    type Item = &'a I;
    fn next(&mut self) -> Option<&'a I> {
        self.0.next()
    }
}
pub trait Interval: Clone + Copy + Debug + Default + Eq + PartialEq + PartialOrd + Ord {
    type Bound: Bound;
    fn lower(&self) -> Self::Bound;
    fn upper(&self) -> Self::Bound;
    fn set_lower(&mut self, bound: Self::Bound);
    fn set_upper(&mut self, bound: Self::Bound);
    fn case_fold_simple(&self, intervals: &mut Vec<Self>) -> Result<(), unicode::CaseFoldError>;
    fn create(lower: Self::Bound, upper: Self::Bound) -> Self {
        let mut int = Self::default();
        if lower <= upper {
            int.set_lower(lower);
            int.set_upper(upper);
        } else {
            int.set_lower(upper);
            int.set_upper(lower);
        }
        int
    }
    fn union(&self, other: &Self) -> Option<Self> {
        if !self.is_contiguous(other) {
            return None;
        }
        let lower = cmp::min(self.lower(), other.lower());
        let upper = cmp::max(self.upper(), other.upper());
        Some(Self::create(lower, upper))
    }
    fn intersect(&self, other: &Self) -> Option<Self> {
        let lower = cmp::max(self.lower(), other.lower());
        let upper = cmp::min(self.upper(), other.upper());
        if lower <= upper {
            Some(Self::create(lower, upper))
        } else {
            None
        }
    }
    fn difference(&self, other: &Self) -> (Option<Self>, Option<Self>) {
        if self.is_subset(other) {
            return (None, None);
        }
        if self.is_intersection_empty(other) {
            return (Some(self.clone()), None);
        }
        let add_lower = other.lower() > self.lower();
        let add_upper = other.upper() < self.upper();
        assert!(add_lower || add_upper);
        let mut ret = (None, None);
        if add_lower {
            let upper = other.lower().decrement();
            ret.0 = Some(Self::create(self.lower(), upper));
        }
        if add_upper {
            let lower = other.upper().increment();
            let range = Self::create(lower, self.upper());
            if ret.0.is_none() {
                ret.0 = Some(range);
            } else {
                ret.1 = Some(range);
            }
        }
        ret
    }
    fn is_contiguous(&self, other: &Self) -> bool {
        let lower1 = self.lower().as_u32();
        let upper1 = self.upper().as_u32();
        let lower2 = other.lower().as_u32();
        let upper2 = other.upper().as_u32();
        cmp::max(lower1, lower2) <= cmp::min(upper1, upper2).saturating_add(1)
    }
    fn is_intersection_empty(&self, other: &Self) -> bool {
        let (lower1, upper1) = (self.lower(), self.upper());
        let (lower2, upper2) = (other.lower(), other.upper());
        cmp::max(lower1, lower2) > cmp::min(upper1, upper2)
    }
    fn is_subset(&self, other: &Self) -> bool {
        let (lower1, upper1) = (self.lower(), self.upper());
        let (lower2, upper2) = (other.lower(), other.upper());
        (lower2 <= lower1 && lower1 <= upper2) && (lower2 <= upper1 && upper1 <= upper2)
    }
}
pub trait Bound: Copy + Clone + Debug + Eq + PartialEq + PartialOrd + Ord {
    fn min_value() -> Self;
    fn max_value() -> Self;
    fn as_u32(self) -> u32;
    fn increment(self) -> Self;
    fn decrement(self) -> Self;
}
impl Bound for u8 {
    fn min_value() -> Self {
        u8::MIN
    }
    fn max_value() -> Self {
        u8::MAX
    }
    fn as_u32(self) -> u32 {
        u32::from(self)
    }
    fn increment(self) -> Self {
        self.checked_add(1).unwrap()
    }
    fn decrement(self) -> Self {
        self.checked_sub(1).unwrap()
    }
}
impl Bound for char {
    fn min_value() -> Self {
        '\x00'
    }
    fn max_value() -> Self {
        '\u{10FFFF}'
    }
    fn as_u32(self) -> u32 {
        u32::from(self)
    }
    fn increment(self) -> Self {
        match self {
            '\u{D7FF}' => '\u{E000}',
            c => char::from_u32(u32::from(c).checked_add(1).unwrap()).unwrap(),
        }
    }
    fn decrement(self) -> Self {
        match self {
            '\u{E000}' => '\u{D7FF}',
            c => char::from_u32(u32::from(c).checked_sub(1).unwrap()).unwrap(),
        }
    }
}
