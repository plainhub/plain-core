use crate::regex_engine::syntax::{
    ast::Span,
    hir::interval::{Interval, IntervalSet, IntervalSetIter},
    unicode,
};
pub use crate::regex_engine::syntax::{
    hir::visitor::{Visitor, visit},
    unicode::CaseFoldError,
};
use core::{char, cmp};
use std::{
    boxed::Box,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
mod interval;
pub mod literal;
pub mod print;
pub mod translate;
mod visitor;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    kind: ErrorKind,
    pattern: String,
    span: Span,
}
impl Error {
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }
    pub fn pattern(&self) -> &str {
        &self.pattern
    }
    pub fn span(&self) -> &Span {
        &self.span
    }
}
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    UnicodeNotAllowed,
    InvalidUtf8,
    InvalidLineTerminator,
    UnicodePropertyNotFound,
    UnicodePropertyValueNotFound,
    UnicodePerlClassNotFound,
    UnicodeCaseUnavailable,
}
impl std::error::Error for Error {}
impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        crate::regex_engine::syntax::error::Formatter::from(self).fmt(f)
    }
}
impl core::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        use self::ErrorKind::*;
        let msg = match *self {
            UnicodeNotAllowed => "Unicode not allowed here",
            InvalidUtf8 => "pattern can match invalid UTF-8",
            InvalidLineTerminator => "invalid line terminator, must be ASCII",
            UnicodePropertyNotFound => "Unicode property not found",
            UnicodePropertyValueNotFound => "Unicode property value not found",
            UnicodePerlClassNotFound => {
                "Unicode-aware Perl class not found \
                 (make sure the unicode-perl feature is enabled)"
            }
            UnicodeCaseUnavailable => {
                "Unicode-aware case insensitivity matching is not available \
                 (make sure the unicode-case feature is enabled)"
            }
        };
        f.write_str(msg)
    }
}
#[derive(Clone, Eq, PartialEq)]
pub struct Hir {
    kind: HirKind,
    props: Properties,
}
impl Hir {
    pub fn kind(&self) -> &HirKind {
        &self.kind
    }
    pub fn into_kind(mut self) -> HirKind {
        core::mem::replace(&mut self.kind, HirKind::Empty)
    }
    pub fn properties(&self) -> &Properties {
        &self.props
    }
    fn into_parts(mut self) -> (HirKind, Properties) {
        (
            core::mem::replace(&mut self.kind, HirKind::Empty),
            core::mem::replace(&mut self.props, Properties::empty()),
        )
    }
}
impl Hir {
    #[inline]
    pub fn empty() -> Hir {
        let props = Properties::empty();
        Hir {
            kind: HirKind::Empty,
            props,
        }
    }
    #[inline]
    pub fn fail() -> Hir {
        let class = Class::Bytes(ClassBytes::empty());
        let props = Properties::class(&class);
        Hir {
            kind: HirKind::Class(class),
            props,
        }
    }
    #[inline]
    pub fn literal<B: Into<Box<[u8]>>>(lit: B) -> Hir {
        let bytes = lit.into();
        if bytes.is_empty() {
            return Hir::empty();
        }
        let lit = Literal(bytes);
        let props = Properties::literal(&lit);
        Hir {
            kind: HirKind::Literal(lit),
            props,
        }
    }
    #[inline]
    pub fn class(class: Class) -> Hir {
        if class.is_empty() {
            return Hir::fail();
        } else if let Some(bytes) = class.literal() {
            return Hir::literal(bytes);
        }
        let props = Properties::class(&class);
        Hir {
            kind: HirKind::Class(class),
            props,
        }
    }
    #[inline]
    pub fn look(look: Look) -> Hir {
        let props = Properties::look(look);
        Hir {
            kind: HirKind::Look(look),
            props,
        }
    }
    #[inline]
    pub fn repetition(mut rep: Repetition) -> Hir {
        if rep.sub.properties().maximum_len() == Some(0) {
            rep.min = cmp::min(rep.min, 1);
            rep.max = rep.max.map(|n| cmp::min(n, 1)).or(Some(1));
        }
        if rep.min == 0 && rep.max == Some(0) {
            return Hir::empty();
        } else if rep.min == 1 && rep.max == Some(1) {
            return *rep.sub;
        }
        let props = Properties::repetition(&rep);
        Hir {
            kind: HirKind::Repetition(rep),
            props,
        }
    }
    #[inline]
    pub fn capture(capture: Capture) -> Hir {
        let props = Properties::capture(&capture);
        Hir {
            kind: HirKind::Capture(capture),
            props,
        }
    }
    pub fn concat(subs: Vec<Hir>) -> Hir {
        let mut new = vec![];
        let mut prior_lit: Option<Vec<u8>> = None;
        for sub in subs {
            let (kind, props) = sub.into_parts();
            match kind {
                HirKind::Literal(Literal(bytes)) => {
                    if let Some(ref mut prior_bytes) = prior_lit {
                        prior_bytes.extend_from_slice(&bytes);
                    } else {
                        prior_lit = Some(bytes.to_vec());
                    }
                }
                HirKind::Concat(subs2) => {
                    for sub2 in subs2 {
                        let (kind2, props2) = sub2.into_parts();
                        match kind2 {
                            HirKind::Literal(Literal(bytes)) => {
                                if let Some(ref mut prior_bytes) = prior_lit {
                                    prior_bytes.extend_from_slice(&bytes);
                                } else {
                                    prior_lit = Some(bytes.to_vec());
                                }
                            }
                            kind2 => {
                                if let Some(prior_bytes) = prior_lit.take() {
                                    new.push(Hir::literal(prior_bytes));
                                }
                                new.push(Hir {
                                    kind: kind2,
                                    props: props2,
                                });
                            }
                        }
                    }
                }
                HirKind::Empty => {}
                kind => {
                    if let Some(prior_bytes) = prior_lit.take() {
                        new.push(Hir::literal(prior_bytes));
                    }
                    new.push(Hir { kind, props });
                }
            }
        }
        if let Some(prior_bytes) = prior_lit.take() {
            new.push(Hir::literal(prior_bytes));
        }
        if new.is_empty() {
            return Hir::empty();
        } else if new.len() == 1 {
            return new.pop().unwrap();
        }
        let props = Properties::concat(&new);
        Hir {
            kind: HirKind::Concat(new),
            props,
        }
    }
    pub fn alternation(subs: Vec<Hir>) -> Hir {
        let mut new = Vec::with_capacity(subs.len());
        for sub in subs {
            let (kind, props) = sub.into_parts();
            match kind {
                HirKind::Alternation(subs2) => {
                    new.extend(subs2);
                }
                kind => {
                    new.push(Hir { kind, props });
                }
            }
        }
        if new.is_empty() {
            return Hir::fail();
        } else if new.len() == 1 {
            return new.pop().unwrap();
        }
        if let Some(singletons) = singleton_chars(&new) {
            let it = singletons
                .into_iter()
                .map(|ch| ClassUnicodeRange { start: ch, end: ch });
            return Hir::class(Class::Unicode(ClassUnicode::new(it)));
        }
        if let Some(singletons) = singleton_bytes(&new) {
            let it = singletons
                .into_iter()
                .map(|b| ClassBytesRange { start: b, end: b });
            return Hir::class(Class::Bytes(ClassBytes::new(it)));
        }
        if let Some(cls) = class_chars(&new) {
            return Hir::class(cls);
        }
        if let Some(cls) = class_bytes(&new) {
            return Hir::class(cls);
        }
        new = match lift_common_prefix(new) {
            Ok(hir) => return hir,
            Err(unchanged) => unchanged,
        };
        let props = Properties::alternation(&new);
        Hir {
            kind: HirKind::Alternation(new),
            props,
        }
    }
    #[inline]
    pub fn dot(dot: Dot) -> Hir {
        match dot {
            Dot::AnyChar => {
                Hir::class(Class::Unicode(ClassUnicode::new([ClassUnicodeRange::new(
                    '\0',
                    '\u{10FFFF}',
                )])))
            }
            Dot::AnyByte => Hir::class(Class::Bytes(ClassBytes::new([ClassBytesRange::new(
                b'\0', b'\xFF',
            )]))),
            Dot::AnyCharExcept(ch) => {
                let mut cls = ClassUnicode::new([ClassUnicodeRange::new(ch, ch)]);
                cls.negate();
                Hir::class(Class::Unicode(cls))
            }
            Dot::AnyCharExceptLF => Hir::class(Class::Unicode(ClassUnicode::new([
                ClassUnicodeRange::new('\0', '\x09'),
                ClassUnicodeRange::new('\x0B', '\u{10FFFF}'),
            ]))),
            Dot::AnyCharExceptCRLF => Hir::class(Class::Unicode(ClassUnicode::new([
                ClassUnicodeRange::new('\0', '\x09'),
                ClassUnicodeRange::new('\x0B', '\x0C'),
                ClassUnicodeRange::new('\x0E', '\u{10FFFF}'),
            ]))),
            Dot::AnyByteExcept(byte) => {
                let mut cls = ClassBytes::new([ClassBytesRange::new(byte, byte)]);
                cls.negate();
                Hir::class(Class::Bytes(cls))
            }
            Dot::AnyByteExceptLF => Hir::class(Class::Bytes(ClassBytes::new([
                ClassBytesRange::new(b'\0', b'\x09'),
                ClassBytesRange::new(b'\x0B', b'\xFF'),
            ]))),
            Dot::AnyByteExceptCRLF => Hir::class(Class::Bytes(ClassBytes::new([
                ClassBytesRange::new(b'\0', b'\x09'),
                ClassBytesRange::new(b'\x0B', b'\x0C'),
                ClassBytesRange::new(b'\x0E', b'\xFF'),
            ]))),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirKind {
    Empty,
    Literal(Literal),
    Class(Class),
    Look(Look),
    Repetition(Repetition),
    Capture(Capture),
    Concat(Vec<Hir>),
    Alternation(Vec<Hir>),
}
impl HirKind {
    pub fn subs(&self) -> &[Hir] {
        use core::slice::from_ref;
        match *self {
            HirKind::Empty | HirKind::Literal(_) | HirKind::Class(_) | HirKind::Look(_) => &[],
            HirKind::Repetition(Repetition { ref sub, .. }) => from_ref(sub),
            HirKind::Capture(Capture { ref sub, .. }) => from_ref(sub),
            HirKind::Concat(ref subs) => subs,
            HirKind::Alternation(ref subs) => subs,
        }
    }
}
impl core::fmt::Debug for Hir {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.kind.fmt(f)
    }
}
impl core::fmt::Display for Hir {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        crate::regex_engine::syntax::hir::print::Printer::new().print(self, f)
    }
}
#[derive(Clone, Eq, PartialEq)]
pub struct Literal(pub Box<[u8]>);
impl core::fmt::Debug for Literal {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        crate::regex_engine::syntax::debug::Bytes(&self.0).fmt(f)
    }
}
#[derive(Clone, Eq, PartialEq)]
pub enum Class {
    Unicode(ClassUnicode),
    Bytes(ClassBytes),
}
impl Class {
    pub fn case_fold_simple(&mut self) {
        match *self {
            Class::Unicode(ref mut x) => x.case_fold_simple(),
            Class::Bytes(ref mut x) => x.case_fold_simple(),
        }
    }
    pub fn try_case_fold_simple(&mut self) -> core::result::Result<(), CaseFoldError> {
        match *self {
            Class::Unicode(ref mut x) => x.try_case_fold_simple()?,
            Class::Bytes(ref mut x) => x.case_fold_simple(),
        }
        Ok(())
    }
    pub fn negate(&mut self) {
        match *self {
            Class::Unicode(ref mut x) => x.negate(),
            Class::Bytes(ref mut x) => x.negate(),
        }
    }
    pub fn is_utf8(&self) -> bool {
        match *self {
            Class::Unicode(_) => true,
            Class::Bytes(ref x) => x.is_ascii(),
        }
    }
    pub fn minimum_len(&self) -> Option<usize> {
        match *self {
            Class::Unicode(ref x) => x.minimum_len(),
            Class::Bytes(ref x) => x.minimum_len(),
        }
    }
    pub fn maximum_len(&self) -> Option<usize> {
        match *self {
            Class::Unicode(ref x) => x.maximum_len(),
            Class::Bytes(ref x) => x.maximum_len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        match *self {
            Class::Unicode(ref x) => x.ranges().is_empty(),
            Class::Bytes(ref x) => x.ranges().is_empty(),
        }
    }
    pub fn literal(&self) -> Option<Vec<u8>> {
        match *self {
            Class::Unicode(ref x) => x.literal(),
            Class::Bytes(ref x) => x.literal(),
        }
    }
}
impl core::fmt::Debug for Class {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use crate::regex_engine::syntax::debug::Byte;
        let mut fmter = f.debug_set();
        match *self {
            Class::Unicode(ref cls) => {
                for r in cls.ranges().iter() {
                    fmter.entry(&(r.start..=r.end));
                }
            }
            Class::Bytes(ref cls) => {
                for r in cls.ranges().iter() {
                    fmter.entry(&(Byte(r.start)..=Byte(r.end)));
                }
            }
        }
        fmter.finish()
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassUnicode {
    set: IntervalSet<ClassUnicodeRange>,
}
impl ClassUnicode {
    pub fn new<I>(ranges: I) -> ClassUnicode
    where
        I: IntoIterator<Item = ClassUnicodeRange>,
    {
        ClassUnicode {
            set: IntervalSet::new(ranges),
        }
    }
    pub fn empty() -> ClassUnicode {
        ClassUnicode::new(vec![])
    }
    pub fn push(&mut self, range: ClassUnicodeRange) {
        self.set.push(range);
    }
    pub fn iter(&self) -> ClassUnicodeIter<'_> {
        ClassUnicodeIter(self.set.iter())
    }
    pub fn ranges(&self) -> &[ClassUnicodeRange] {
        self.set.intervals()
    }
    pub fn case_fold_simple(&mut self) {
        self.set
            .case_fold_simple()
            .expect("unicode-case feature must be enabled");
    }
    pub fn try_case_fold_simple(&mut self) -> core::result::Result<(), CaseFoldError> {
        self.set.case_fold_simple()
    }
    pub fn negate(&mut self) {
        self.set.negate();
    }
    pub fn union(&mut self, other: &ClassUnicode) {
        self.set.union(&other.set);
    }
    pub fn intersect(&mut self, other: &ClassUnicode) {
        self.set.intersect(&other.set);
    }
    pub fn difference(&mut self, other: &ClassUnicode) {
        self.set.difference(&other.set);
    }
    pub fn symmetric_difference(&mut self, other: &ClassUnicode) {
        self.set.symmetric_difference(&other.set);
    }
    pub fn is_ascii(&self) -> bool {
        self.set
            .intervals()
            .last()
            .map_or(true, |r| r.end <= '\x7F')
    }
    pub fn minimum_len(&self) -> Option<usize> {
        let first = self.ranges().get(0)?;
        Some(first.start.len_utf8())
    }
    pub fn maximum_len(&self) -> Option<usize> {
        let last = self.ranges().last()?;
        Some(last.end.len_utf8())
    }
    pub fn literal(&self) -> Option<Vec<u8>> {
        let rs = self.ranges();
        if rs.len() == 1 && rs[0].start == rs[0].end {
            Some(
                rs[0]
                    .start
                    .encode_utf8(&mut [0; 4])
                    .to_string()
                    .into_bytes(),
            )
        } else {
            None
        }
    }
    pub fn to_byte_class(&self) -> Option<ClassBytes> {
        if !self.is_ascii() {
            return None;
        }
        Some(ClassBytes::new(self.ranges().iter().map(|r| {
            ClassBytesRange {
                start: u8::try_from(r.start).unwrap(),
                end: u8::try_from(r.end).unwrap(),
            }
        })))
    }
}
#[derive(Debug)]
pub struct ClassUnicodeIter<'a>(IntervalSetIter<'a, ClassUnicodeRange>);
impl<'a> Iterator for ClassUnicodeIter<'a> {
    type Item = &'a ClassUnicodeRange;
    fn next(&mut self) -> Option<&'a ClassUnicodeRange> {
        self.0.next()
    }
}
#[derive(Clone, Copy, Default, Eq, PartialEq, PartialOrd, Ord)]
pub struct ClassUnicodeRange {
    start: char,
    end: char,
}
impl core::fmt::Debug for ClassUnicodeRange {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let start = if !self.start.is_whitespace() && !self.start.is_control() {
            self.start.to_string()
        } else {
            format!("0x{:X}", u32::from(self.start))
        };
        let end = if !self.end.is_whitespace() && !self.end.is_control() {
            self.end.to_string()
        } else {
            format!("0x{:X}", u32::from(self.end))
        };
        f.debug_struct("ClassUnicodeRange")
            .field("start", &start)
            .field("end", &end)
            .finish()
    }
}
impl Interval for ClassUnicodeRange {
    type Bound = char;
    #[inline]
    fn lower(&self) -> char {
        self.start
    }
    #[inline]
    fn upper(&self) -> char {
        self.end
    }
    #[inline]
    fn set_lower(&mut self, bound: char) {
        self.start = bound;
    }
    #[inline]
    fn set_upper(&mut self, bound: char) {
        self.end = bound;
    }
    fn case_fold_simple(
        &self,
        ranges: &mut Vec<ClassUnicodeRange>,
    ) -> Result<(), unicode::CaseFoldError> {
        let mut folder = unicode::SimpleCaseFolder::new()?;
        if !folder.overlaps(self.start, self.end) {
            return Ok(());
        }
        let (start, end) = (u32::from(self.start), u32::from(self.end));
        for cp in (start..=end).filter_map(char::from_u32) {
            for &cp_folded in folder.mapping(cp) {
                ranges.push(ClassUnicodeRange::new(cp_folded, cp_folded));
            }
        }
        Ok(())
    }
}
impl ClassUnicodeRange {
    pub fn new(start: char, end: char) -> ClassUnicodeRange {
        ClassUnicodeRange::create(start, end)
    }
    pub fn start(&self) -> char {
        self.start
    }
    pub fn end(&self) -> char {
        self.end
    }
    pub fn len(&self) -> usize {
        let diff = 1 + u32::from(self.end) - u32::from(self.start);
        usize::try_from(diff).expect("char class len fits in usize")
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassBytes {
    set: IntervalSet<ClassBytesRange>,
}
impl ClassBytes {
    pub fn new<I>(ranges: I) -> ClassBytes
    where
        I: IntoIterator<Item = ClassBytesRange>,
    {
        ClassBytes {
            set: IntervalSet::new(ranges),
        }
    }
    pub fn empty() -> ClassBytes {
        ClassBytes::new(vec![])
    }
    pub fn push(&mut self, range: ClassBytesRange) {
        self.set.push(range);
    }
    pub fn iter(&self) -> ClassBytesIter<'_> {
        ClassBytesIter(self.set.iter())
    }
    pub fn ranges(&self) -> &[ClassBytesRange] {
        self.set.intervals()
    }
    pub fn case_fold_simple(&mut self) {
        self.set
            .case_fold_simple()
            .expect("ASCII case folding never fails");
    }
    pub fn negate(&mut self) {
        self.set.negate();
    }
    pub fn union(&mut self, other: &ClassBytes) {
        self.set.union(&other.set);
    }
    pub fn intersect(&mut self, other: &ClassBytes) {
        self.set.intersect(&other.set);
    }
    pub fn difference(&mut self, other: &ClassBytes) {
        self.set.difference(&other.set);
    }
    pub fn symmetric_difference(&mut self, other: &ClassBytes) {
        self.set.symmetric_difference(&other.set);
    }
    pub fn is_ascii(&self) -> bool {
        self.set.intervals().last().map_or(true, |r| r.end <= 0x7F)
    }
    pub fn minimum_len(&self) -> Option<usize> {
        if self.ranges().is_empty() {
            None
        } else {
            Some(1)
        }
    }
    pub fn maximum_len(&self) -> Option<usize> {
        if self.ranges().is_empty() {
            None
        } else {
            Some(1)
        }
    }
    pub fn literal(&self) -> Option<Vec<u8>> {
        let rs = self.ranges();
        if rs.len() == 1 && rs[0].start == rs[0].end {
            Some(vec![rs[0].start])
        } else {
            None
        }
    }
    pub fn to_unicode_class(&self) -> Option<ClassUnicode> {
        if !self.is_ascii() {
            return None;
        }
        Some(ClassUnicode::new(self.ranges().iter().map(|r| {
            ClassUnicodeRange {
                start: char::from(r.start),
                end: char::from(r.end),
            }
        })))
    }
}
#[derive(Debug)]
pub struct ClassBytesIter<'a>(IntervalSetIter<'a, ClassBytesRange>);
impl<'a> Iterator for ClassBytesIter<'a> {
    type Item = &'a ClassBytesRange;
    fn next(&mut self) -> Option<&'a ClassBytesRange> {
        self.0.next()
    }
}
#[derive(Clone, Copy, Default, Eq, PartialEq, PartialOrd, Ord)]
pub struct ClassBytesRange {
    start: u8,
    end: u8,
}
impl Interval for ClassBytesRange {
    type Bound = u8;
    #[inline]
    fn lower(&self) -> u8 {
        self.start
    }
    #[inline]
    fn upper(&self) -> u8 {
        self.end
    }
    #[inline]
    fn set_lower(&mut self, bound: u8) {
        self.start = bound;
    }
    #[inline]
    fn set_upper(&mut self, bound: u8) {
        self.end = bound;
    }
    fn case_fold_simple(
        &self,
        ranges: &mut Vec<ClassBytesRange>,
    ) -> Result<(), unicode::CaseFoldError> {
        if !ClassBytesRange::new(b'a', b'z').is_intersection_empty(self) {
            let lower = cmp::max(self.start, b'a');
            let upper = cmp::min(self.end, b'z');
            ranges.push(ClassBytesRange::new(lower - 32, upper - 32));
        }
        if !ClassBytesRange::new(b'A', b'Z').is_intersection_empty(self) {
            let lower = cmp::max(self.start, b'A');
            let upper = cmp::min(self.end, b'Z');
            ranges.push(ClassBytesRange::new(lower + 32, upper + 32));
        }
        Ok(())
    }
}
impl ClassBytesRange {
    pub fn new(start: u8, end: u8) -> ClassBytesRange {
        ClassBytesRange::create(start, end)
    }
    pub fn start(&self) -> u8 {
        self.start
    }
    pub fn end(&self) -> u8 {
        self.end
    }
    pub fn len(&self) -> usize {
        usize::from(self.end.checked_sub(self.start).unwrap())
            .checked_add(1)
            .unwrap()
    }
}
impl core::fmt::Debug for ClassBytesRange {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ClassBytesRange")
            .field(
                "start",
                &crate::regex_engine::syntax::debug::Byte(self.start),
            )
            .field("end", &crate::regex_engine::syntax::debug::Byte(self.end))
            .finish()
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Look {
    Start = 1 << 0,
    End = 1 << 1,
    StartLF = 1 << 2,
    EndLF = 1 << 3,
    StartCRLF = 1 << 4,
    EndCRLF = 1 << 5,
    WordAscii = 1 << 6,
    WordAsciiNegate = 1 << 7,
    WordUnicode = 1 << 8,
    WordUnicodeNegate = 1 << 9,
    WordStartAscii = 1 << 10,
    WordEndAscii = 1 << 11,
    WordStartUnicode = 1 << 12,
    WordEndUnicode = 1 << 13,
    WordStartHalfAscii = 1 << 14,
    WordEndHalfAscii = 1 << 15,
    WordStartHalfUnicode = 1 << 16,
    WordEndHalfUnicode = 1 << 17,
}
impl Look {
    #[inline]
    pub const fn reversed(self) -> Look {
        match self {
            Look::Start => Look::End,
            Look::End => Look::Start,
            Look::StartLF => Look::EndLF,
            Look::EndLF => Look::StartLF,
            Look::StartCRLF => Look::EndCRLF,
            Look::EndCRLF => Look::StartCRLF,
            Look::WordAscii => Look::WordAscii,
            Look::WordAsciiNegate => Look::WordAsciiNegate,
            Look::WordUnicode => Look::WordUnicode,
            Look::WordUnicodeNegate => Look::WordUnicodeNegate,
            Look::WordStartAscii => Look::WordEndAscii,
            Look::WordEndAscii => Look::WordStartAscii,
            Look::WordStartUnicode => Look::WordEndUnicode,
            Look::WordEndUnicode => Look::WordStartUnicode,
            Look::WordStartHalfAscii => Look::WordEndHalfAscii,
            Look::WordEndHalfAscii => Look::WordStartHalfAscii,
            Look::WordStartHalfUnicode => Look::WordEndHalfUnicode,
            Look::WordEndHalfUnicode => Look::WordStartHalfUnicode,
        }
    }
    #[inline]
    pub const fn as_repr(self) -> u32 {
        self as u32
    }
    #[inline]
    pub const fn from_repr(repr: u32) -> Option<Look> {
        match repr {
            0b00_0000_0000_0000_0001 => Some(Look::Start),
            0b00_0000_0000_0000_0010 => Some(Look::End),
            0b00_0000_0000_0000_0100 => Some(Look::StartLF),
            0b00_0000_0000_0000_1000 => Some(Look::EndLF),
            0b00_0000_0000_0001_0000 => Some(Look::StartCRLF),
            0b00_0000_0000_0010_0000 => Some(Look::EndCRLF),
            0b00_0000_0000_0100_0000 => Some(Look::WordAscii),
            0b00_0000_0000_1000_0000 => Some(Look::WordAsciiNegate),
            0b00_0000_0001_0000_0000 => Some(Look::WordUnicode),
            0b00_0000_0010_0000_0000 => Some(Look::WordUnicodeNegate),
            0b00_0000_0100_0000_0000 => Some(Look::WordStartAscii),
            0b00_0000_1000_0000_0000 => Some(Look::WordEndAscii),
            0b00_0001_0000_0000_0000 => Some(Look::WordStartUnicode),
            0b00_0010_0000_0000_0000 => Some(Look::WordEndUnicode),
            0b00_0100_0000_0000_0000 => Some(Look::WordStartHalfAscii),
            0b00_1000_0000_0000_0000 => Some(Look::WordEndHalfAscii),
            0b01_0000_0000_0000_0000 => Some(Look::WordStartHalfUnicode),
            0b10_0000_0000_0000_0000 => Some(Look::WordEndHalfUnicode),
            _ => None,
        }
    }
    #[inline]
    pub const fn as_char(self) -> char {
        match self {
            Look::Start => 'A',
            Look::End => 'z',
            Look::StartLF => '^',
            Look::EndLF => '$',
            Look::StartCRLF => 'r',
            Look::EndCRLF => 'R',
            Look::WordAscii => 'b',
            Look::WordAsciiNegate => 'B',
            Look::WordUnicode => '𝛃',
            Look::WordUnicodeNegate => '𝚩',
            Look::WordStartAscii => '<',
            Look::WordEndAscii => '>',
            Look::WordStartUnicode => '〈',
            Look::WordEndUnicode => '〉',
            Look::WordStartHalfAscii => '◁',
            Look::WordEndHalfAscii => '▷',
            Look::WordStartHalfUnicode => '◀',
            Look::WordEndHalfUnicode => '▶',
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Capture {
    pub index: u32,
    pub name: Option<Box<str>>,
    pub sub: Box<Hir>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Repetition {
    pub min: u32,
    pub max: Option<u32>,
    pub greedy: bool,
    pub sub: Box<Hir>,
}
impl Repetition {
    pub fn with(&self, sub: Hir) -> Repetition {
        Repetition {
            min: self.min,
            max: self.max,
            greedy: self.greedy,
            sub: Box::new(sub),
        }
    }
}
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Dot {
    AnyChar,
    AnyByte,
    AnyCharExcept(char),
    AnyCharExceptLF,
    AnyCharExceptCRLF,
    AnyByteExcept(u8),
    AnyByteExceptLF,
    AnyByteExceptCRLF,
}
impl Drop for Hir {
    fn drop(&mut self) {
        use core::mem;
        match *self.kind() {
            HirKind::Empty | HirKind::Literal(_) | HirKind::Class(_) | HirKind::Look(_) => return,
            HirKind::Capture(ref x) if x.sub.kind.subs().is_empty() => return,
            HirKind::Repetition(ref x) if x.sub.kind.subs().is_empty() => return,
            HirKind::Concat(ref x) if x.is_empty() => return,
            HirKind::Alternation(ref x) if x.is_empty() => return,
            _ => {}
        }
        let mut stack = vec![mem::replace(self, Hir::empty())];
        while let Some(mut expr) = stack.pop() {
            match expr.kind {
                HirKind::Empty | HirKind::Literal(_) | HirKind::Class(_) | HirKind::Look(_) => {}
                HirKind::Capture(ref mut x) => {
                    stack.push(mem::replace(&mut x.sub, Hir::empty()));
                }
                HirKind::Repetition(ref mut x) => {
                    stack.push(mem::replace(&mut x.sub, Hir::empty()));
                }
                HirKind::Concat(ref mut x) => {
                    stack.extend(x.drain(..));
                }
                HirKind::Alternation(ref mut x) => {
                    stack.extend(x.drain(..));
                }
            }
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Properties(Box<PropertiesI>);
#[derive(Clone, Debug, Eq, PartialEq)]
struct PropertiesI {
    minimum_len: Option<usize>,
    maximum_len: Option<usize>,
    look_set: LookSet,
    look_set_prefix: LookSet,
    look_set_suffix: LookSet,
    look_set_prefix_any: LookSet,
    look_set_suffix_any: LookSet,
    utf8: bool,
    explicit_captures_len: usize,
    static_explicit_captures_len: Option<usize>,
    literal: bool,
    alternation_literal: bool,
}
impl Properties {
    #[inline]
    pub fn minimum_len(&self) -> Option<usize> {
        self.0.minimum_len
    }
    #[inline]
    pub fn maximum_len(&self) -> Option<usize> {
        self.0.maximum_len
    }
    #[inline]
    pub fn look_set(&self) -> LookSet {
        self.0.look_set
    }
    #[inline]
    pub fn look_set_prefix(&self) -> LookSet {
        self.0.look_set_prefix
    }
    #[inline]
    pub fn look_set_prefix_any(&self) -> LookSet {
        self.0.look_set_prefix_any
    }
    #[inline]
    pub fn look_set_suffix(&self) -> LookSet {
        self.0.look_set_suffix
    }
    #[inline]
    pub fn look_set_suffix_any(&self) -> LookSet {
        self.0.look_set_suffix_any
    }
    #[inline]
    pub fn is_utf8(&self) -> bool {
        self.0.utf8
    }
    #[inline]
    pub fn explicit_captures_len(&self) -> usize {
        self.0.explicit_captures_len
    }
    #[inline]
    pub fn static_explicit_captures_len(&self) -> Option<usize> {
        self.0.static_explicit_captures_len
    }
    #[inline]
    pub fn is_literal(&self) -> bool {
        self.0.literal
    }
    #[inline]
    pub fn is_alternation_literal(&self) -> bool {
        self.0.alternation_literal
    }
    #[inline]
    pub fn memory_usage(&self) -> usize {
        core::mem::size_of::<PropertiesI>()
    }
    pub fn union<I, P>(props: I) -> Properties
    where
        I: IntoIterator<Item = P>,
        P: core::borrow::Borrow<Properties>,
    {
        let mut it = props.into_iter().peekable();
        let fix = if it.peek().is_none() {
            LookSet::empty()
        } else {
            LookSet::full()
        };
        let static_explicit_captures_len = it
            .peek()
            .and_then(|p| p.borrow().static_explicit_captures_len());
        let mut props = PropertiesI {
            minimum_len: None,
            maximum_len: None,
            look_set: LookSet::empty(),
            look_set_prefix: fix,
            look_set_suffix: fix,
            look_set_prefix_any: LookSet::empty(),
            look_set_suffix_any: LookSet::empty(),
            utf8: true,
            explicit_captures_len: 0,
            static_explicit_captures_len,
            literal: false,
            alternation_literal: true,
        };
        let (mut min_poisoned, mut max_poisoned) = (false, false);
        for prop in it {
            let p = prop.borrow();
            props.look_set.set_union(p.look_set());
            props.look_set_prefix.set_intersect(p.look_set_prefix());
            props.look_set_suffix.set_intersect(p.look_set_suffix());
            props.look_set_prefix_any.set_union(p.look_set_prefix_any());
            props.look_set_suffix_any.set_union(p.look_set_suffix_any());
            props.utf8 = props.utf8 && p.is_utf8();
            props.explicit_captures_len = props
                .explicit_captures_len
                .saturating_add(p.explicit_captures_len());
            if props.static_explicit_captures_len != p.static_explicit_captures_len() {
                props.static_explicit_captures_len = None;
            }
            props.alternation_literal = props.alternation_literal && p.is_literal();
            if !min_poisoned {
                if let Some(xmin) = p.minimum_len() {
                    if props.minimum_len.map_or(true, |pmin| xmin < pmin) {
                        props.minimum_len = Some(xmin);
                    }
                } else {
                    props.minimum_len = None;
                    min_poisoned = true;
                }
            }
            if !max_poisoned {
                if let Some(xmax) = p.maximum_len() {
                    if props.maximum_len.map_or(true, |pmax| xmax > pmax) {
                        props.maximum_len = Some(xmax);
                    }
                } else {
                    props.maximum_len = None;
                    max_poisoned = true;
                }
            }
        }
        Properties(Box::new(props))
    }
}
impl Properties {
    fn empty() -> Properties {
        let inner = PropertiesI {
            minimum_len: Some(0),
            maximum_len: Some(0),
            look_set: LookSet::empty(),
            look_set_prefix: LookSet::empty(),
            look_set_suffix: LookSet::empty(),
            look_set_prefix_any: LookSet::empty(),
            look_set_suffix_any: LookSet::empty(),
            utf8: true,
            explicit_captures_len: 0,
            static_explicit_captures_len: Some(0),
            literal: false,
            alternation_literal: false,
        };
        Properties(Box::new(inner))
    }
    fn literal(lit: &Literal) -> Properties {
        let inner = PropertiesI {
            minimum_len: Some(lit.0.len()),
            maximum_len: Some(lit.0.len()),
            look_set: LookSet::empty(),
            look_set_prefix: LookSet::empty(),
            look_set_suffix: LookSet::empty(),
            look_set_prefix_any: LookSet::empty(),
            look_set_suffix_any: LookSet::empty(),
            utf8: core::str::from_utf8(&lit.0).is_ok(),
            explicit_captures_len: 0,
            static_explicit_captures_len: Some(0),
            literal: true,
            alternation_literal: true,
        };
        Properties(Box::new(inner))
    }
    fn class(class: &Class) -> Properties {
        let inner = PropertiesI {
            minimum_len: class.minimum_len(),
            maximum_len: class.maximum_len(),
            look_set: LookSet::empty(),
            look_set_prefix: LookSet::empty(),
            look_set_suffix: LookSet::empty(),
            look_set_prefix_any: LookSet::empty(),
            look_set_suffix_any: LookSet::empty(),
            utf8: class.is_utf8(),
            explicit_captures_len: 0,
            static_explicit_captures_len: Some(0),
            literal: false,
            alternation_literal: false,
        };
        Properties(Box::new(inner))
    }
    fn look(look: Look) -> Properties {
        let inner = PropertiesI {
            minimum_len: Some(0),
            maximum_len: Some(0),
            look_set: LookSet::singleton(look),
            look_set_prefix: LookSet::singleton(look),
            look_set_suffix: LookSet::singleton(look),
            look_set_prefix_any: LookSet::singleton(look),
            look_set_suffix_any: LookSet::singleton(look),
            utf8: true,
            explicit_captures_len: 0,
            static_explicit_captures_len: Some(0),
            literal: false,
            alternation_literal: false,
        };
        Properties(Box::new(inner))
    }
    fn repetition(rep: &Repetition) -> Properties {
        let p = rep.sub.properties();
        let minimum_len = p.minimum_len().map(|child_min| {
            let rep_min = usize::try_from(rep.min).unwrap_or(usize::MAX);
            child_min.saturating_mul(rep_min)
        });
        let maximum_len = rep.max.and_then(|rep_max| {
            let rep_max = usize::try_from(rep_max).ok()?;
            let child_max = p.maximum_len()?;
            child_max.checked_mul(rep_max)
        });
        let mut inner = PropertiesI {
            minimum_len,
            maximum_len,
            look_set: p.look_set(),
            look_set_prefix: LookSet::empty(),
            look_set_suffix: LookSet::empty(),
            look_set_prefix_any: p.look_set_prefix_any(),
            look_set_suffix_any: p.look_set_suffix_any(),
            utf8: p.is_utf8(),
            explicit_captures_len: p.explicit_captures_len(),
            static_explicit_captures_len: p.static_explicit_captures_len(),
            literal: false,
            alternation_literal: false,
        };
        if rep.min > 0 {
            inner.look_set_prefix = p.look_set_prefix();
            inner.look_set_suffix = p.look_set_suffix();
        }
        if rep.min == 0
            && inner
                .static_explicit_captures_len
                .map_or(false, |len| len > 0)
        {
            if rep.max == Some(0) {
                inner.static_explicit_captures_len = Some(0);
            } else {
                inner.static_explicit_captures_len = None;
            }
        }
        Properties(Box::new(inner))
    }
    fn capture(capture: &Capture) -> Properties {
        let p = capture.sub.properties();
        Properties(Box::new(PropertiesI {
            explicit_captures_len: p.explicit_captures_len().saturating_add(1),
            static_explicit_captures_len: p
                .static_explicit_captures_len()
                .map(|len| len.saturating_add(1)),
            literal: false,
            alternation_literal: false,
            ..*p.0.clone()
        }))
    }
    fn concat(concat: &[Hir]) -> Properties {
        let mut props = PropertiesI {
            minimum_len: Some(0),
            maximum_len: Some(0),
            look_set: LookSet::empty(),
            look_set_prefix: LookSet::empty(),
            look_set_suffix: LookSet::empty(),
            look_set_prefix_any: LookSet::empty(),
            look_set_suffix_any: LookSet::empty(),
            utf8: true,
            explicit_captures_len: 0,
            static_explicit_captures_len: Some(0),
            literal: true,
            alternation_literal: true,
        };
        for x in concat.iter() {
            let p = x.properties();
            props.look_set.set_union(p.look_set());
            props.utf8 = props.utf8 && p.is_utf8();
            props.explicit_captures_len = props
                .explicit_captures_len
                .saturating_add(p.explicit_captures_len());
            props.static_explicit_captures_len = p
                .static_explicit_captures_len()
                .and_then(|len1| Some((len1, props.static_explicit_captures_len?)))
                .and_then(|(len1, len2)| Some(len1.saturating_add(len2)));
            props.literal = props.literal && p.is_literal();
            props.alternation_literal = props.alternation_literal && p.is_alternation_literal();
            if let Some(minimum_len) = props.minimum_len {
                match p.minimum_len() {
                    None => props.minimum_len = None,
                    Some(len) => {
                        props.minimum_len = Some(minimum_len.saturating_add(len));
                    }
                }
            }
            if let Some(maximum_len) = props.maximum_len {
                match p.maximum_len() {
                    None => props.maximum_len = None,
                    Some(len) => props.maximum_len = maximum_len.checked_add(len),
                }
            }
        }
        let mut it = concat.iter();
        while let Some(x) = it.next() {
            props
                .look_set_prefix
                .set_union(x.properties().look_set_prefix());
            props
                .look_set_prefix_any
                .set_union(x.properties().look_set_prefix_any());
            if x.properties().maximum_len().map_or(true, |x| x > 0) {
                break;
            }
        }
        let mut it = concat.iter().rev();
        while let Some(x) = it.next() {
            props
                .look_set_suffix
                .set_union(x.properties().look_set_suffix());
            props
                .look_set_suffix_any
                .set_union(x.properties().look_set_suffix_any());
            if x.properties().maximum_len().map_or(true, |x| x > 0) {
                break;
            }
        }
        Properties(Box::new(props))
    }
    fn alternation(alts: &[Hir]) -> Properties {
        Properties::union(alts.iter().map(|hir| hir.properties()))
    }
}
#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub struct LookSet {
    pub bits: u32,
}
impl LookSet {
    #[inline]
    pub fn empty() -> LookSet {
        LookSet { bits: 0 }
    }
    #[inline]
    pub fn full() -> LookSet {
        LookSet { bits: !0 }
    }
    #[inline]
    pub fn singleton(look: Look) -> LookSet {
        LookSet::empty().insert(look)
    }
    #[inline]
    pub fn len(self) -> usize {
        usize::try_from(self.bits.count_ones()).unwrap()
    }
    #[inline]
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    #[inline]
    pub fn contains(self, look: Look) -> bool {
        self.bits & look.as_repr() != 0
    }
    #[inline]
    pub fn contains_anchor(&self) -> bool {
        self.contains_anchor_haystack() || self.contains_anchor_line()
    }
    #[inline]
    pub fn contains_anchor_haystack(&self) -> bool {
        self.contains(Look::Start) || self.contains(Look::End)
    }
    #[inline]
    pub fn contains_anchor_line(&self) -> bool {
        self.contains(Look::StartLF)
            || self.contains(Look::EndLF)
            || self.contains(Look::StartCRLF)
            || self.contains(Look::EndCRLF)
    }
    #[inline]
    pub fn contains_anchor_lf(&self) -> bool {
        self.contains(Look::StartLF) || self.contains(Look::EndLF)
    }
    #[inline]
    pub fn contains_anchor_crlf(&self) -> bool {
        self.contains(Look::StartCRLF) || self.contains(Look::EndCRLF)
    }
    #[inline]
    pub fn contains_word(self) -> bool {
        self.contains_word_unicode() || self.contains_word_ascii()
    }
    #[inline]
    pub fn contains_word_unicode(self) -> bool {
        self.contains(Look::WordUnicode)
            || self.contains(Look::WordUnicodeNegate)
            || self.contains(Look::WordStartUnicode)
            || self.contains(Look::WordEndUnicode)
            || self.contains(Look::WordStartHalfUnicode)
            || self.contains(Look::WordEndHalfUnicode)
    }
    #[inline]
    pub fn contains_word_ascii(self) -> bool {
        self.contains(Look::WordAscii)
            || self.contains(Look::WordAsciiNegate)
            || self.contains(Look::WordStartAscii)
            || self.contains(Look::WordEndAscii)
            || self.contains(Look::WordStartHalfAscii)
            || self.contains(Look::WordEndHalfAscii)
    }
    #[inline]
    pub fn iter(self) -> LookSetIter {
        LookSetIter { set: self }
    }
    #[inline]
    pub fn insert(self, look: Look) -> LookSet {
        LookSet {
            bits: self.bits | look.as_repr(),
        }
    }
    #[inline]
    pub fn set_insert(&mut self, look: Look) {
        *self = self.insert(look);
    }
    #[inline]
    pub fn remove(self, look: Look) -> LookSet {
        LookSet {
            bits: self.bits & !look.as_repr(),
        }
    }
    #[inline]
    pub fn set_remove(&mut self, look: Look) {
        *self = self.remove(look);
    }
    #[inline]
    pub fn subtract(self, other: LookSet) -> LookSet {
        LookSet {
            bits: self.bits & !other.bits,
        }
    }
    #[inline]
    pub fn set_subtract(&mut self, other: LookSet) {
        *self = self.subtract(other);
    }
    #[inline]
    pub fn union(self, other: LookSet) -> LookSet {
        LookSet {
            bits: self.bits | other.bits,
        }
    }
    #[inline]
    pub fn set_union(&mut self, other: LookSet) {
        *self = self.union(other);
    }
    #[inline]
    pub fn intersect(self, other: LookSet) -> LookSet {
        LookSet {
            bits: self.bits & other.bits,
        }
    }
    #[inline]
    pub fn set_intersect(&mut self, other: LookSet) {
        *self = self.intersect(other);
    }
    #[inline]
    pub fn read_repr(slice: &[u8]) -> LookSet {
        let bits = u32::from_ne_bytes(slice[..4].try_into().unwrap());
        LookSet { bits }
    }
    #[inline]
    pub fn write_repr(self, slice: &mut [u8]) {
        let raw = self.bits.to_ne_bytes();
        slice[0] = raw[0];
        slice[1] = raw[1];
        slice[2] = raw[2];
        slice[3] = raw[3];
    }
}
impl core::fmt::Debug for LookSet {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        if self.is_empty() {
            return write!(f, "∅");
        }
        for look in self.iter() {
            write!(f, "{}", look.as_char())?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct LookSetIter {
    set: LookSet,
}
impl Iterator for LookSetIter {
    type Item = Look;
    #[inline]
    fn next(&mut self) -> Option<Look> {
        if self.set.is_empty() {
            return None;
        }
        let bit = u16::try_from(self.set.bits.trailing_zeros()).unwrap();
        let look = Look::from_repr(1 << bit)?;
        self.set = self.set.remove(look);
        Some(look)
    }
}
fn class_chars(hirs: &[Hir]) -> Option<Class> {
    let mut cls = ClassUnicode::new(vec![]);
    for hir in hirs.iter() {
        match *hir.kind() {
            HirKind::Class(Class::Unicode(ref cls2)) => {
                cls.union(cls2);
            }
            HirKind::Class(Class::Bytes(ref cls2)) => {
                cls.union(&cls2.to_unicode_class()?);
            }
            _ => return None,
        };
    }
    Some(Class::Unicode(cls))
}
fn class_bytes(hirs: &[Hir]) -> Option<Class> {
    let mut cls = ClassBytes::new(vec![]);
    for hir in hirs.iter() {
        match *hir.kind() {
            HirKind::Class(Class::Unicode(ref cls2)) => {
                cls.union(&cls2.to_byte_class()?);
            }
            HirKind::Class(Class::Bytes(ref cls2)) => {
                cls.union(cls2);
            }
            _ => return None,
        };
    }
    Some(Class::Bytes(cls))
}
fn singleton_chars(hirs: &[Hir]) -> Option<Vec<char>> {
    let mut singletons = vec![];
    for hir in hirs.iter() {
        let literal = match *hir.kind() {
            HirKind::Literal(Literal(ref bytes)) => bytes,
            _ => return None,
        };
        let ch = match crate::regex_engine::syntax::debug::utf8_decode(literal) {
            None => return None,
            Some(Err(_)) => return None,
            Some(Ok(ch)) => ch,
        };
        if literal.len() != ch.len_utf8() {
            return None;
        }
        singletons.push(ch);
    }
    Some(singletons)
}
fn singleton_bytes(hirs: &[Hir]) -> Option<Vec<u8>> {
    let mut singletons = vec![];
    for hir in hirs.iter() {
        let literal = match *hir.kind() {
            HirKind::Literal(Literal(ref bytes)) => bytes,
            _ => return None,
        };
        if literal.len() != 1 {
            return None;
        }
        singletons.push(literal[0]);
    }
    Some(singletons)
}
fn lift_common_prefix(hirs: Vec<Hir>) -> Result<Hir, Vec<Hir>> {
    if hirs.len() <= 1 {
        return Err(hirs);
    }
    let mut prefix = match hirs[0].kind() {
        HirKind::Concat(xs) => &**xs,
        _ => return Err(hirs),
    };
    if prefix.is_empty() {
        return Err(hirs);
    }
    for h in hirs.iter().skip(1) {
        let concat = match h.kind() {
            HirKind::Concat(xs) => xs,
            _ => return Err(hirs),
        };
        let common_len = prefix
            .iter()
            .zip(concat.iter())
            .take_while(|(x, y)| x == y)
            .count();
        prefix = &prefix[..common_len];
        if prefix.is_empty() {
            return Err(hirs);
        }
    }
    let len = prefix.len();
    assert_ne!(0, len);
    let mut prefix_concat = vec![];
    let mut suffix_alts = vec![];
    for h in hirs {
        let mut concat = match h.into_kind() {
            HirKind::Concat(xs) => xs,
            _ => unreachable!(),
        };
        suffix_alts.push(Hir::concat(concat.split_off(len)));
        if prefix_concat.is_empty() {
            prefix_concat = concat;
        }
    }
    let mut concat = prefix_concat;
    concat.push(Hir::alternation(suffix_alts));
    Ok(Hir::concat(concat))
}
