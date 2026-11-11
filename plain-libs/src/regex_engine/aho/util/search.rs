use crate::regex_engine::aho::util::primitives::PatternID;
use core::ops::{Range, RangeBounds};
#[derive(Clone)]
pub struct Input<'h> {
    haystack: &'h [u8],
    span: Span,
    anchored: Anchored,
    earliest: bool,
}
impl<'h> Input<'h> {
    #[inline]
    pub fn new<H: ?Sized + AsRef<[u8]>>(haystack: &'h H) -> Input<'h> {
        Input {
            haystack: haystack.as_ref(),
            span: Span {
                start: 0,
                end: haystack.as_ref().len(),
            },
            anchored: Anchored::No,
            earliest: false,
        }
    }
    #[inline]
    pub fn span<S: Into<Span>>(mut self, span: S) -> Input<'h> {
        self.set_span(span);
        self
    }
    #[inline]
    pub fn range<R: RangeBounds<usize>>(mut self, range: R) -> Input<'h> {
        self.set_range(range);
        self
    }
    #[inline]
    pub fn anchored(mut self, mode: Anchored) -> Input<'h> {
        self.set_anchored(mode);
        self
    }
    #[inline]
    pub fn earliest(mut self, yes: bool) -> Input<'h> {
        self.set_earliest(yes);
        self
    }
    #[inline]
    pub fn set_span<S: Into<Span>>(&mut self, span: S) {
        let span = span.into();
        assert!(
            span.end <= self.haystack.len() && span.start <= span.end.wrapping_add(1),
            "invalid span {:?} for haystack of length {}",
            span,
            self.haystack.len(),
        );
        self.span = span;
    }
    #[inline]
    pub fn set_range<R: RangeBounds<usize>>(&mut self, range: R) {
        use core::ops::Bound;
        let start = match range.start_bound() {
            Bound::Included(&i) => i,
            Bound::Excluded(&i) => i.checked_add(1).unwrap(),
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(&i) => i.checked_add(1).unwrap(),
            Bound::Excluded(&i) => i,
            Bound::Unbounded => self.haystack().len(),
        };
        self.set_span(Span { start, end });
    }
    #[inline]
    pub fn set_start(&mut self, start: usize) {
        self.set_span(Span {
            start,
            ..self.get_span()
        });
    }
    #[inline]
    pub fn set_end(&mut self, end: usize) {
        self.set_span(Span {
            end,
            ..self.get_span()
        });
    }
    #[inline]
    pub fn set_anchored(&mut self, mode: Anchored) {
        self.anchored = mode;
    }
    #[inline]
    pub fn set_earliest(&mut self, yes: bool) {
        self.earliest = yes;
    }
    #[inline]
    pub fn haystack(&self) -> &[u8] {
        self.haystack
    }
    #[inline]
    pub fn start(&self) -> usize {
        self.get_span().start
    }
    #[inline]
    pub fn end(&self) -> usize {
        self.get_span().end
    }
    #[inline]
    pub fn get_span(&self) -> Span {
        self.span
    }
    #[inline]
    pub fn get_range(&self) -> Range<usize> {
        self.get_span().range()
    }
    #[inline]
    pub fn get_anchored(&self) -> Anchored {
        self.anchored
    }
    #[inline]
    pub fn get_earliest(&self) -> bool {
        self.earliest
    }
    #[inline]
    pub fn is_done(&self) -> bool {
        self.get_span().start > self.get_span().end
    }
}
impl<'h> core::fmt::Debug for Input<'h> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let mut fmter = f.debug_struct("Input");
        match core::str::from_utf8(self.haystack()) {
            Ok(nice) => fmter.field("haystack", &nice),
            Err(_) => fmter.field("haystack", &self.haystack()),
        }
        .field("span", &self.span)
        .field("anchored", &self.anchored)
        .field("earliest", &self.earliest)
        .finish()
    }
}
impl<'h, H: ?Sized + AsRef<[u8]>> From<&'h H> for Input<'h> {
    #[inline]
    fn from(haystack: &'h H) -> Input<'h> {
        Input::new(haystack)
    }
}
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}
impl Span {
    #[inline]
    pub fn range(&self) -> Range<usize> {
        Range::from(*self)
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }
    #[inline]
    pub fn contains(&self, offset: usize) -> bool {
        !self.is_empty() && self.start <= offset && offset <= self.end
    }
    #[inline]
    pub fn offset(&self, offset: usize) -> Span {
        Span {
            start: self
                .start
                .checked_add(offset)
                .expect("invalid start+offset"),
            end: self.end.checked_add(offset).expect("invalid end+offset"),
        }
    }
}
impl core::fmt::Debug for Span {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}
impl core::ops::Index<Span> for [u8] {
    type Output = [u8];
    #[inline]
    fn index(&self, index: Span) -> &[u8] {
        &self[index.range()]
    }
}
impl core::ops::IndexMut<Span> for [u8] {
    #[inline]
    fn index_mut(&mut self, index: Span) -> &mut [u8] {
        &mut self[index.range()]
    }
}
impl core::ops::Index<Span> for str {
    type Output = str;
    #[inline]
    fn index(&self, index: Span) -> &str {
        &self[index.range()]
    }
}
impl From<Range<usize>> for Span {
    #[inline]
    fn from(range: Range<usize>) -> Span {
        Span {
            start: range.start,
            end: range.end,
        }
    }
}
impl From<Span> for Range<usize> {
    #[inline]
    fn from(span: Span) -> Range<usize> {
        Range {
            start: span.start,
            end: span.end,
        }
    }
}
impl PartialEq<Range<usize>> for Span {
    #[inline]
    fn eq(&self, range: &Range<usize>) -> bool {
        self.start == range.start && self.end == range.end
    }
}
impl PartialEq<Span> for Range<usize> {
    #[inline]
    fn eq(&self, span: &Span) -> bool {
        self.start == span.start && self.end == span.end
    }
}
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Anchored {
    No,
    Yes,
}
impl Anchored {
    #[inline]
    pub fn is_anchored(&self) -> bool {
        matches!(*self, Anchored::Yes)
    }
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Match {
    pattern: PatternID,
    span: Span,
}
impl Match {
    #[inline]
    pub fn new<S: Into<Span>>(pattern: PatternID, span: S) -> Match {
        let span = span.into();
        assert!(span.start <= span.end, "invalid match span");
        Match { pattern, span }
    }
    #[inline]
    pub fn must<S: Into<Span>>(pattern: usize, span: S) -> Match {
        Match::new(PatternID::must(pattern), span)
    }
    #[inline]
    pub fn pattern(&self) -> PatternID {
        self.pattern
    }
    #[inline]
    pub fn start(&self) -> usize {
        self.span().start
    }
    #[inline]
    pub fn end(&self) -> usize {
        self.span().end
    }
    #[inline]
    pub fn range(&self) -> core::ops::Range<usize> {
        self.span().range()
    }
    #[inline]
    pub fn span(&self) -> Span {
        self.span
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.span().is_empty()
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.span().len()
    }
    #[inline]
    pub fn offset(&self, offset: usize) -> Match {
        Match {
            pattern: self.pattern,
            span: self.span.offset(offset),
        }
    }
}
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MatchKind {
    Standard,
    LeftmostFirst,
    LeftmostLongest,
}
impl Default for MatchKind {
    fn default() -> MatchKind {
        MatchKind::Standard
    }
}
impl MatchKind {
    #[inline]
    pub(crate) fn is_standard(&self) -> bool {
        matches!(*self, MatchKind::Standard)
    }
    #[inline]
    pub(crate) fn is_leftmost(&self) -> bool {
        matches!(*self, MatchKind::LeftmostFirst | MatchKind::LeftmostLongest)
    }
    #[inline]
    pub(crate) fn is_leftmost_first(&self) -> bool {
        matches!(*self, MatchKind::LeftmostFirst)
    }
    #[inline]
    pub(crate) fn as_packed(&self) -> Option<crate::regex_engine::aho::packed::MatchKind> {
        match *self {
            MatchKind::Standard => None,
            MatchKind::LeftmostFirst => {
                Some(crate::regex_engine::aho::packed::MatchKind::LeftmostFirst)
            }
            MatchKind::LeftmostLongest => {
                Some(crate::regex_engine::aho::packed::MatchKind::LeftmostLongest)
            }
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartKind {
    Both,
    Unanchored,
    Anchored,
}
impl Default for StartKind {
    fn default() -> StartKind {
        StartKind::Unanchored
    }
}
