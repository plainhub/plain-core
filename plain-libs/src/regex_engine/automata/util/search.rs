use crate::regex_engine::automata::util::{escape::DebugByte, primitives::PatternID, utf8};
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
        let haystack = haystack.as_ref();
        Input {
            haystack,
            span: Span {
                start: 0,
                end: haystack.len(),
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
    pub fn haystack(&self) -> &'h [u8] {
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
    #[inline]
    pub fn is_char_boundary(&self, offset: usize) -> bool {
        utf8::is_boundary(self.haystack(), offset)
    }
}
impl<'h> core::fmt::Debug for Input<'h> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use crate::regex_engine::automata::util::escape::DebugHaystack;
        f.debug_struct("Input")
            .field("haystack", &DebugHaystack(self.haystack()))
            .field("span", &self.span)
            .field("anchored", &self.anchored)
            .field("earliest", &self.earliest)
            .finish()
    }
}
impl<'h, H: ?Sized + AsRef<[u8]>> From<&'h H> for Input<'h> {
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
            start: self.start + offset,
            end: self.end + offset,
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
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HalfMatch {
    pattern: PatternID,
    offset: usize,
}
impl HalfMatch {
    #[inline]
    pub fn new(pattern: PatternID, offset: usize) -> HalfMatch {
        HalfMatch { pattern, offset }
    }
    #[inline]
    pub fn must(pattern: usize, offset: usize) -> HalfMatch {
        HalfMatch::new(PatternID::new(pattern).unwrap(), offset)
    }
    #[inline]
    pub fn pattern(&self) -> PatternID {
        self.pattern
    }
    #[inline]
    pub fn offset(&self) -> usize {
        self.offset
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
        let span: Span = span.into();
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
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatternSet {
    len: usize,
    which: std::boxed::Box<[bool]>,
}
impl PatternSet {
    pub fn new(capacity: usize) -> PatternSet {
        assert!(
            capacity <= PatternID::LIMIT,
            "pattern set capacity exceeds limit of {}",
            PatternID::LIMIT,
        );
        PatternSet {
            len: 0,
            which: std :: vec ! [false ; capacity].into_boxed_slice(),
        }
    }
    pub fn clear(&mut self) {
        self.len = 0;
        for matched in self.which.iter_mut() {
            *matched = false;
        }
    }
    pub fn contains(&self, pid: PatternID) -> bool {
        pid.as_usize() < self.capacity() && self.which[pid]
    }
    pub fn insert(&mut self, pid: PatternID) -> bool {
        self.try_insert(pid)
            .expect("PatternSet should have sufficient capacity")
    }
    pub fn try_insert(&mut self, pid: PatternID) -> Result<bool, PatternSetInsertError> {
        if pid.as_usize() >= self.capacity() {
            return Err(PatternSetInsertError {
                attempted: pid,
                capacity: self.capacity(),
            });
        }
        if self.which[pid] {
            return Ok(false);
        }
        self.len += 1;
        self.which[pid] = true;
        Ok(true)
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn is_full(&self) -> bool {
        self.len() == self.capacity()
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn capacity(&self) -> usize {
        self.which.len()
    }
    pub fn iter(&self) -> PatternSetIter<'_> {
        PatternSetIter {
            it: self.which.iter().enumerate(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct PatternSetInsertError {
    attempted: PatternID,
    capacity: usize,
}
impl std::error::Error for PatternSetInsertError {}
impl core::fmt::Display for PatternSetInsertError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(
            f,
            "failed to insert pattern ID {} into pattern set \
             with insufficient capacity of {}",
            self.attempted.as_usize(),
            self.capacity,
        )
    }
}
#[derive(Clone, Debug)]
pub struct PatternSetIter<'a> {
    it: core::iter::Enumerate<core::slice::Iter<'a, bool>>,
}
impl<'a> Iterator for PatternSetIter<'a> {
    type Item = PatternID;
    fn next(&mut self) -> Option<PatternID> {
        while let Some((index, &yes)) = self.it.next() {
            if yes {
                return Some(PatternID::new_unchecked(index));
            }
        }
        None
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }
}
impl<'a> DoubleEndedIterator for PatternSetIter<'a> {
    fn next_back(&mut self) -> Option<PatternID> {
        while let Some((index, &yes)) = self.it.next_back() {
            if yes {
                return Some(PatternID::new_unchecked(index));
            }
        }
        None
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Anchored {
    No,
    Yes,
    Pattern(PatternID),
}
impl Anchored {
    #[inline]
    pub fn is_anchored(&self) -> bool {
        matches!(*self, Anchored::Yes | Anchored::Pattern(_))
    }
    #[inline]
    pub fn pattern(&self) -> Option<PatternID> {
        match *self {
            Anchored::Pattern(pid) => Some(pid),
            _ => None,
        }
    }
}
#[non_exhaustive]
#[derive(Clone, Copy, Default, Debug, Eq, PartialEq)]
pub enum MatchKind {
    All,
    #[default]
    LeftmostFirst,
}
impl MatchKind {
    pub(crate) fn continue_past_first_match(&self) -> bool {
        *self == MatchKind::All
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchError(
    #[cfg(all())] std::boxed::Box<MatchErrorKind>,
    #[cfg(not(all()))] MatchErrorKind,
);
impl MatchError {
    pub fn new(kind: MatchErrorKind) -> MatchError {
        { MatchError(std::boxed::Box::new(kind)) }
    }
    pub fn kind(&self) -> &MatchErrorKind {
        &self.0
    }
    pub fn quit(byte: u8, offset: usize) -> MatchError {
        MatchError::new(MatchErrorKind::Quit { byte, offset })
    }
    pub fn gave_up(offset: usize) -> MatchError {
        MatchError::new(MatchErrorKind::GaveUp { offset })
    }
    pub fn haystack_too_long(len: usize) -> MatchError {
        MatchError::new(MatchErrorKind::HaystackTooLong { len })
    }
    pub fn unsupported_anchored(mode: Anchored) -> MatchError {
        MatchError::new(MatchErrorKind::UnsupportedAnchored { mode })
    }
}
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MatchErrorKind {
    Quit { byte: u8, offset: usize },
    GaveUp { offset: usize },
    HaystackTooLong { len: usize },
    UnsupportedAnchored { mode: Anchored },
}
impl std::error::Error for MatchError {}
impl core::fmt::Display for MatchError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match *self.kind() {
            MatchErrorKind::Quit { byte, offset } => write!(
                f,
                "quit search after observing byte {:?} at offset {}",
                DebugByte(byte),
                offset,
            ),
            MatchErrorKind::GaveUp { offset } => {
                write!(f, "gave up searching at offset {offset}")
            }
            MatchErrorKind::HaystackTooLong { len } => {
                write!(f, "haystack of length {len} is too long")
            }
            MatchErrorKind::UnsupportedAnchored {
                mode: Anchored::Yes,
            } => {
                write!(f, "anchored searches are not supported or enabled")
            }
            MatchErrorKind::UnsupportedAnchored { mode: Anchored::No } => {
                write!(f, "unanchored searches are not supported or enabled")
            }
            MatchErrorKind::UnsupportedAnchored {
                mode: Anchored::Pattern(pid),
            } => {
                write!(
                    f,
                    "anchored searches for a specific pattern ({}) are \
                     not supported or enabled",
                    pid.as_usize(),
                )
            }
        }
    }
}
