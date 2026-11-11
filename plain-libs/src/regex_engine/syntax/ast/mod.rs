pub use crate::regex_engine::syntax::ast::visitor::{Visitor, visit};
use core::cmp::Ordering;
use std::{boxed::Box, string::String, vec, vec::Vec};
pub mod parse;
pub mod print;
mod visitor;
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
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
    pub fn auxiliary_span(&self) -> Option<&Span> {
        use self::ErrorKind::*;
        match self.kind {
            FlagDuplicate { ref original } => Some(original),
            FlagRepeatedNegation { ref original, .. } => Some(original),
            GroupNameDuplicate { ref original, .. } => Some(original),
            _ => None,
        }
    }
}
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub enum ErrorKind {
    CaptureLimitExceeded,
    ClassEscapeInvalid,
    ClassRangeInvalid,
    ClassRangeLiteral,
    ClassUnclosed,
    DecimalEmpty,
    DecimalInvalid,
    EscapeHexEmpty,
    EscapeHexInvalid,
    EscapeHexInvalidDigit,
    EscapeUnexpectedEof,
    EscapeUnrecognized,
    FlagDanglingNegation,
    FlagDuplicate { original: Span },
    FlagRepeatedNegation { original: Span },
    FlagUnexpectedEof,
    FlagUnrecognized,
    GroupNameDuplicate { original: Span },
    GroupNameEmpty,
    GroupNameInvalid,
    GroupNameUnexpectedEof,
    GroupUnclosed,
    GroupUnopened,
    NestLimitExceeded(u32),
    RepetitionCountInvalid,
    RepetitionCountDecimalEmpty,
    RepetitionCountUnclosed,
    RepetitionMissing,
    SpecialWordBoundaryUnclosed,
    SpecialWordBoundaryUnrecognized,
    SpecialWordOrRepetitionUnexpectedEof,
    UnicodeClassInvalid,
    UnsupportedBackreference,
    UnsupportedLookAround,
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
        match *self {
            CaptureLimitExceeded => write!(
                f,
                "exceeded the maximum number of \
                 capturing groups ({})",
                u32::MAX
            ),
            ClassEscapeInvalid => {
                write!(f, "invalid escape sequence found in character class")
            }
            ClassRangeInvalid => write!(
                f,
                "invalid character class range, \
                 the start must be <= the end"
            ),
            ClassRangeLiteral => {
                write!(f, "invalid range boundary, must be a literal")
            }
            ClassUnclosed => write!(f, "unclosed character class"),
            DecimalEmpty => write!(f, "decimal literal empty"),
            DecimalInvalid => write!(f, "decimal literal invalid"),
            EscapeHexEmpty => write!(f, "hexadecimal literal empty"),
            EscapeHexInvalid => {
                write!(f, "hexadecimal literal is not a Unicode scalar value")
            }
            EscapeHexInvalidDigit => write!(f, "invalid hexadecimal digit"),
            EscapeUnexpectedEof => write!(
                f,
                "incomplete escape sequence, \
                 reached end of pattern prematurely"
            ),
            EscapeUnrecognized => write!(f, "unrecognized escape sequence"),
            FlagDanglingNegation => {
                write!(f, "dangling flag negation operator")
            }
            FlagDuplicate { .. } => write!(f, "duplicate flag"),
            FlagRepeatedNegation { .. } => {
                write!(f, "flag negation operator repeated")
            }
            FlagUnexpectedEof => {
                write!(f, "expected flag but got end of regex")
            }
            FlagUnrecognized => write!(f, "unrecognized flag"),
            GroupNameDuplicate { .. } => {
                write!(f, "duplicate capture group name")
            }
            GroupNameEmpty => write!(f, "empty capture group name"),
            GroupNameInvalid => write!(f, "invalid capture group character"),
            GroupNameUnexpectedEof => write!(f, "unclosed capture group name"),
            GroupUnclosed => write!(f, "unclosed group"),
            GroupUnopened => write!(f, "unopened group"),
            NestLimitExceeded(limit) => write!(
                f,
                "exceed the maximum number of \
                 nested parentheses/brackets ({})",
                limit
            ),
            RepetitionCountInvalid => write!(
                f,
                "invalid repetition count range, \
                 the start must be <= the end"
            ),
            RepetitionCountDecimalEmpty => {
                write!(f, "repetition quantifier expects a valid decimal")
            }
            RepetitionCountUnclosed => {
                write!(f, "unclosed counted repetition")
            }
            RepetitionMissing => {
                write!(f, "repetition operator missing expression")
            }
            SpecialWordBoundaryUnclosed => {
                write!(
                    f,
                    "special word boundary assertion is either \
                     unclosed or contains an invalid character",
                )
            }
            SpecialWordBoundaryUnrecognized => {
                write!(
                    f,
                    "unrecognized special word boundary assertion, \
                     valid choices are: start, end, start-half \
                     or end-half",
                )
            }
            SpecialWordOrRepetitionUnexpectedEof => {
                write!(
                    f,
                    "found either the beginning of a special word \
                     boundary or a bounded repetition on a \\b with \
                     an opening brace, but no closing brace",
                )
            }
            UnicodeClassInvalid => {
                write!(f, "invalid Unicode character class")
            }
            UnsupportedBackreference => {
                write!(f, "backreferences are not supported")
            }
            UnsupportedLookAround => write!(
                f,
                "look-around, including look-ahead and look-behind, \
                 is not supported"
            ),
        }
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub struct Span {
    pub start: Position,
    pub end: Position,
}
impl core::fmt::Debug for Span {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Span({:?}, {:?})", self.start, self.end)
    }
}
impl Ord for Span {
    fn cmp(&self, other: &Span) -> Ordering {
        (&self.start, &self.end).cmp(&(&other.start, &other.end))
    }
}
impl PartialOrd for Span {
    fn partial_cmp(&self, other: &Span) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub struct Position {
    pub offset: usize,
    pub line: usize,
    pub column: usize,
}
impl core::fmt::Debug for Position {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Position(o: {:?}, l: {:?}, c: {:?})",
            self.offset, self.line, self.column
        )
    }
}
impl Ord for Position {
    fn cmp(&self, other: &Position) -> Ordering {
        self.offset.cmp(&other.offset)
    }
}
impl PartialOrd for Position {
    fn partial_cmp(&self, other: &Position) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Span {
    pub fn new(start: Position, end: Position) -> Span {
        Span { start, end }
    }
    pub fn splat(pos: Position) -> Span {
        Span::new(pos, pos)
    }

    pub fn with_end(self, pos: Position) -> Span {
        Span { end: pos, ..self }
    }
    pub fn is_one_line(&self) -> bool {
        self.start.line == self.end.line
    }
    pub fn is_empty(&self) -> bool {
        self.start.offset == self.end.offset
    }
}
impl Position {
    pub fn new(offset: usize, line: usize, column: usize) -> Position {
        Position {
            offset,
            line,
            column,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub struct WithComments {
    pub ast: Ast,
    pub comments: Vec<Comment>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub struct Comment {
    pub span: Span,
    pub comment: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub enum Ast {
    Empty(Box<Span>),
    Flags(Box<SetFlags>),
    Literal(Box<Literal>),
    Dot(Box<Span>),
    Assertion(Box<Assertion>),
    ClassUnicode(Box<ClassUnicode>),
    ClassPerl(Box<ClassPerl>),
    ClassBracketed(Box<ClassBracketed>),
    Repetition(Box<Repetition>),
    Group(Box<Group>),
    Alternation(Box<Alternation>),
    Concat(Box<Concat>),
}
impl Ast {
    pub fn empty(span: Span) -> Ast {
        Ast::Empty(Box::new(span))
    }
    pub fn flags(e: SetFlags) -> Ast {
        Ast::Flags(Box::new(e))
    }
    pub fn literal(e: Literal) -> Ast {
        Ast::Literal(Box::new(e))
    }
    pub fn dot(span: Span) -> Ast {
        Ast::Dot(Box::new(span))
    }
    pub fn assertion(e: Assertion) -> Ast {
        Ast::Assertion(Box::new(e))
    }
    pub fn class_unicode(e: ClassUnicode) -> Ast {
        Ast::ClassUnicode(Box::new(e))
    }
    pub fn class_perl(e: ClassPerl) -> Ast {
        Ast::ClassPerl(Box::new(e))
    }
    pub fn class_bracketed(e: ClassBracketed) -> Ast {
        Ast::ClassBracketed(Box::new(e))
    }
    pub fn repetition(e: Repetition) -> Ast {
        Ast::Repetition(Box::new(e))
    }
    pub fn group(e: Group) -> Ast {
        Ast::Group(Box::new(e))
    }
    pub fn alternation(e: Alternation) -> Ast {
        Ast::Alternation(Box::new(e))
    }
    pub fn concat(e: Concat) -> Ast {
        Ast::Concat(Box::new(e))
    }
    pub fn span(&self) -> &Span {
        match *self {
            Ast::Empty(ref span) => span,
            Ast::Flags(ref x) => &x.span,
            Ast::Literal(ref x) => &x.span,
            Ast::Dot(ref span) => span,
            Ast::Assertion(ref x) => &x.span,
            Ast::ClassUnicode(ref x) => &x.span,
            Ast::ClassPerl(ref x) => &x.span,
            Ast::ClassBracketed(ref x) => &x.span,
            Ast::Repetition(ref x) => &x.span,
            Ast::Group(ref x) => &x.span,
            Ast::Alternation(ref x) => &x.span,
            Ast::Concat(ref x) => &x.span,
        }
    }
    pub fn is_empty(&self) -> bool {
        match *self {
            Ast::Empty(_) => true,
            _ => false,
        }
    }
    fn has_subexprs(&self) -> bool {
        match *self {
            Ast::Empty(_)
            | Ast::Flags(_)
            | Ast::Literal(_)
            | Ast::Dot(_)
            | Ast::Assertion(_)
            | Ast::ClassUnicode(_)
            | Ast::ClassPerl(_) => false,
            Ast::ClassBracketed(_)
            | Ast::Repetition(_)
            | Ast::Group(_)
            | Ast::Alternation(_)
            | Ast::Concat(_) => true,
        }
    }
}
impl core::fmt::Display for Ast {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        use crate::regex_engine::syntax::ast::print::Printer;
        Printer::new().print(self, f)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub struct Alternation {
    pub span: Span,
    pub asts: Vec<Ast>,
}
impl Alternation {
    pub fn into_ast(mut self) -> Ast {
        match self.asts.len() {
            0 => Ast::empty(self.span),
            1 => self.asts.pop().unwrap(),
            _ => Ast::alternation(self),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub struct Concat {
    pub span: Span,
    pub asts: Vec<Ast>,
}
impl Concat {
    pub fn into_ast(mut self) -> Ast {
        match self.asts.len() {
            0 => Ast::empty(self.span),
            1 => self.asts.pop().unwrap(),
            _ => Ast::concat(self),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub struct Literal {
    pub span: Span,
    pub kind: LiteralKind,
    pub c: char,
}
impl Literal {
    pub fn byte(&self) -> Option<u8> {
        match self.kind {
            LiteralKind::HexFixed(HexLiteralKind::X) => u8::try_from(self.c).ok(),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub enum LiteralKind {
    Verbatim,
    Meta,
    Superfluous,
    Octal,
    HexFixed(HexLiteralKind),
    HexBrace(HexLiteralKind),
    Special(SpecialLiteralKind),
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub enum SpecialLiteralKind {
    Bell,
    FormFeed,
    Tab,
    LineFeed,
    CarriageReturn,
    VerticalTab,
    Space,
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub enum HexLiteralKind {
    X,
    UnicodeShort,
    UnicodeLong,
}
impl HexLiteralKind {
    pub fn digits(&self) -> u32 {
        match *self {
            HexLiteralKind::X => 2,
            HexLiteralKind::UnicodeShort => 4,
            HexLiteralKind::UnicodeLong => 8,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub struct ClassPerl {
    pub span: Span,
    pub kind: ClassPerlKind,
    pub negated: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub enum ClassPerlKind {
    Digit,
    Space,
    Word,
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub struct ClassAscii {
    pub span: Span,
    pub kind: ClassAsciiKind,
    pub negated: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub enum ClassAsciiKind {
    Alnum,
    Alpha,
    Ascii,
    Blank,
    Cntrl,
    Digit,
    Graph,
    Lower,
    Print,
    Punct,
    Space,
    Upper,
    Word,
    Xdigit,
}
impl ClassAsciiKind {
    pub fn from_name(name: &str) -> Option<ClassAsciiKind> {
        use self::ClassAsciiKind::*;
        match name {
            "alnum" => Some(Alnum),
            "alpha" => Some(Alpha),
            "ascii" => Some(Ascii),
            "blank" => Some(Blank),
            "cntrl" => Some(Cntrl),
            "digit" => Some(Digit),
            "graph" => Some(Graph),
            "lower" => Some(Lower),
            "print" => Some(Print),
            "punct" => Some(Punct),
            "space" => Some(Space),
            "upper" => Some(Upper),
            "word" => Some(Word),
            "xdigit" => Some(Xdigit),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub struct ClassUnicode {
    pub span: Span,
    pub negated: bool,
    pub kind: ClassUnicodeKind,
}
impl ClassUnicode {
    pub fn is_negated(&self) -> bool {
        match self.kind {
            ClassUnicodeKind::NamedValue {
                op: ClassUnicodeOpKind::NotEqual,
                ..
            } => !self.negated,
            _ => self.negated,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClassUnicodeKind {
    OneLetter(char),
    Named(String),
    NamedValue {
        op: ClassUnicodeOpKind,
        name: String,
        value: String,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub enum ClassUnicodeOpKind {
    Equal,
    Colon,
    NotEqual,
}
impl ClassUnicodeOpKind {
    pub fn is_equal(&self) -> bool {
        match *self {
            ClassUnicodeOpKind::Equal | ClassUnicodeOpKind::Colon => true,
            _ => false,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub struct ClassBracketed {
    pub span: Span,
    pub negated: bool,
    pub kind: ClassSet,
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub enum ClassSet {
    Item(ClassSetItem),
    BinaryOp(ClassSetBinaryOp),
}
impl ClassSet {
    pub fn union(ast: ClassSetUnion) -> ClassSet {
        ClassSet::Item(ClassSetItem::Union(ast))
    }
    pub fn span(&self) -> &Span {
        match *self {
            ClassSet::Item(ref x) => x.span(),
            ClassSet::BinaryOp(ref x) => &x.span,
        }
    }
    fn is_empty(&self) -> bool {
        match *self {
            ClassSet::Item(ClassSetItem::Empty(_)) => true,
            _ => false,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub enum ClassSetItem {
    Empty(Span),
    Literal(Literal),
    Range(ClassSetRange),
    Ascii(ClassAscii),
    Unicode(ClassUnicode),
    Perl(ClassPerl),
    Bracketed(Box<ClassBracketed>),
    Union(ClassSetUnion),
}
impl ClassSetItem {
    pub fn span(&self) -> &Span {
        match *self {
            ClassSetItem::Empty(ref span) => span,
            ClassSetItem::Literal(ref x) => &x.span,
            ClassSetItem::Range(ref x) => &x.span,
            ClassSetItem::Ascii(ref x) => &x.span,
            ClassSetItem::Perl(ref x) => &x.span,
            ClassSetItem::Unicode(ref x) => &x.span,
            ClassSetItem::Bracketed(ref x) => &x.span,
            ClassSetItem::Union(ref x) => &x.span,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub struct ClassSetRange {
    pub span: Span,
    pub start: Literal,
    pub end: Literal,
}
impl ClassSetRange {
    pub fn is_valid(&self) -> bool {
        self.start.c <= self.end.c
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub struct ClassSetUnion {
    pub span: Span,
    pub items: Vec<ClassSetItem>,
}
impl ClassSetUnion {
    pub fn push(&mut self, item: ClassSetItem) {
        if self.items.is_empty() {
            self.span.start = item.span().start;
        }
        self.span.end = item.span().end;
        self.items.push(item);
    }
    pub fn into_item(mut self) -> ClassSetItem {
        match self.items.len() {
            0 => ClassSetItem::Empty(self.span),
            1 => self.items.pop().unwrap(),
            _ => ClassSetItem::Union(self),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub struct ClassSetBinaryOp {
    pub span: Span,
    pub kind: ClassSetBinaryOpKind,
    pub lhs: Box<ClassSet>,
    pub rhs: Box<ClassSet>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub enum ClassSetBinaryOpKind {
    Intersection,
    Difference,
    SymmetricDifference,
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub struct Assertion {
    pub span: Span,
    pub kind: AssertionKind,
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub enum AssertionKind {
    StartLine,
    EndLine,
    StartText,
    EndText,
    WordBoundary,
    NotWordBoundary,
    WordBoundaryStart,
    WordBoundaryEnd,
    WordBoundaryStartAngle,
    WordBoundaryEndAngle,
    WordBoundaryStartHalf,
    WordBoundaryEndHalf,
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub struct Repetition {
    pub span: Span,
    pub op: RepetitionOp,
    pub greedy: bool,
    pub ast: Box<Ast>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub struct RepetitionOp {
    pub span: Span,
    pub kind: RepetitionKind,
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub enum RepetitionKind {
    ZeroOrOne,
    ZeroOrMore,
    OneOrMore,
    Range(RepetitionRange),
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub enum RepetitionRange {
    Exactly(u32),
    AtLeast(u32),
    Bounded(u32, u32),
}
impl RepetitionRange {
    pub fn is_valid(&self) -> bool {
        match *self {
            RepetitionRange::Bounded(s, e) if s > e => false,
            _ => true,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub struct Group {
    pub span: Span,
    pub kind: GroupKind,
    pub ast: Box<Ast>,
}
impl Group {
    pub fn flags(&self) -> Option<&Flags> {
        match self.kind {
            GroupKind::NonCapturing(ref flags) => Some(flags),
            _ => None,
        }
    }

    pub fn capture_index(&self) -> Option<u32> {
        match self.kind {
            GroupKind::CaptureIndex(i) => Some(i),
            GroupKind::CaptureName { ref name, .. } => Some(name.index),
            GroupKind::NonCapturing(_) => None,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub enum GroupKind {
    CaptureIndex(u32),
    CaptureName {
        starts_with_p: bool,
        name: CaptureName,
    },
    NonCapturing(Flags),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaptureName {
    pub span: Span,
    pub name: String,
    pub index: u32,
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub struct SetFlags {
    pub span: Span,
    pub flags: Flags,
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub struct Flags {
    pub span: Span,
    pub items: Vec<FlagsItem>,
}
impl Flags {
    pub fn add_item(&mut self, item: FlagsItem) -> Option<usize> {
        for (i, x) in self.items.iter().enumerate() {
            if x.kind == item.kind {
                return Some(i);
            }
        }
        self.items.push(item);
        None
    }
    pub fn flag_state(&self, flag: Flag) -> Option<bool> {
        let mut negated = false;
        for x in &self.items {
            match x.kind {
                FlagsItemKind::Negation => {
                    negated = true;
                }
                FlagsItemKind::Flag(ref xflag) if xflag == &flag => {
                    return Some(!negated);
                }
                _ => {}
            }
        }
        None
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub struct FlagsItem {
    pub span: Span,
    pub kind: FlagsItemKind,
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub enum FlagsItemKind {
    Negation,
    Flag(Flag),
}
impl FlagsItemKind {}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
pub enum Flag {
    CaseInsensitive,
    MultiLine,
    DotMatchesNewLine,
    SwapGreed,
    Unicode,
    CRLF,
    IgnoreWhitespace,
}
impl Drop for Ast {
    fn drop(&mut self) {
        use core::mem;
        match *self {
            Ast::Empty(_)
            | Ast::Flags(_)
            | Ast::Literal(_)
            | Ast::Dot(_)
            | Ast::Assertion(_)
            | Ast::ClassUnicode(_)
            | Ast::ClassPerl(_)
            | Ast::ClassBracketed(_) => return,
            Ast::Repetition(ref x) if !x.ast.has_subexprs() => return,
            Ast::Group(ref x) if !x.ast.has_subexprs() => return,
            Ast::Alternation(ref x) if x.asts.is_empty() => return,
            Ast::Concat(ref x) if x.asts.is_empty() => return,
            _ => {}
        }
        let empty_span = || Span::splat(Position::new(0, 0, 0));
        let empty_ast = || Ast::empty(empty_span());
        let mut stack = vec![mem::replace(self, empty_ast())];
        while let Some(mut ast) = stack.pop() {
            match ast {
                Ast::Empty(_)
                | Ast::Flags(_)
                | Ast::Literal(_)
                | Ast::Dot(_)
                | Ast::Assertion(_)
                | Ast::ClassUnicode(_)
                | Ast::ClassPerl(_)
                | Ast::ClassBracketed(_) => {}
                Ast::Repetition(ref mut x) => {
                    stack.push(mem::replace(&mut x.ast, empty_ast()));
                }
                Ast::Group(ref mut x) => {
                    stack.push(mem::replace(&mut x.ast, empty_ast()));
                }
                Ast::Alternation(ref mut x) => {
                    stack.extend(x.asts.drain(..));
                }
                Ast::Concat(ref mut x) => {
                    stack.extend(x.asts.drain(..));
                }
            }
        }
    }
}
impl Drop for ClassSet {
    fn drop(&mut self) {
        use core::mem;
        match *self {
            ClassSet::Item(ref item) => match *item {
                ClassSetItem::Empty(_)
                | ClassSetItem::Literal(_)
                | ClassSetItem::Range(_)
                | ClassSetItem::Ascii(_)
                | ClassSetItem::Unicode(_)
                | ClassSetItem::Perl(_) => return,
                ClassSetItem::Bracketed(ref x) => {
                    if x.kind.is_empty() {
                        return;
                    }
                }
                ClassSetItem::Union(ref x) => {
                    if x.items.is_empty() {
                        return;
                    }
                }
            },
            ClassSet::BinaryOp(ref op) => {
                if op.lhs.is_empty() && op.rhs.is_empty() {
                    return;
                }
            }
        }
        let empty_span = || Span::splat(Position::new(0, 0, 0));
        let empty_set = || ClassSet::Item(ClassSetItem::Empty(empty_span()));
        let mut stack = vec![mem::replace(self, empty_set())];
        while let Some(mut set) = stack.pop() {
            match set {
                ClassSet::Item(ref mut item) => match *item {
                    ClassSetItem::Empty(_)
                    | ClassSetItem::Literal(_)
                    | ClassSetItem::Range(_)
                    | ClassSetItem::Ascii(_)
                    | ClassSetItem::Unicode(_)
                    | ClassSetItem::Perl(_) => {}
                    ClassSetItem::Bracketed(ref mut x) => {
                        stack.push(mem::replace(&mut x.kind, empty_set()));
                    }
                    ClassSetItem::Union(ref mut x) => {
                        stack.extend(x.items.drain(..).map(ClassSet::Item));
                    }
                },
                ClassSet::BinaryOp(ref mut op) => {
                    stack.push(mem::replace(&mut op.lhs, empty_set()));
                    stack.push(mem::replace(&mut op.rhs, empty_set()));
                }
            }
        }
    }
}
