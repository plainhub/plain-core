use crate::regex_engine::automata::util::{escape::DebugByte, utf8};
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
    pub fn available(self) -> Result<(), UnicodeWordBoundaryError> {
        if self.contains_word_unicode() {
            UnicodeWordBoundaryError::check()?;
        }
        Ok(())
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
#[derive(Clone, Debug)]
pub struct LookMatcher {
    lineterm: DebugByte,
}
impl LookMatcher {
    pub fn new() -> LookMatcher {
        LookMatcher {
            lineterm: DebugByte(b'\n'),
        }
    }
    pub fn set_line_terminator(&mut self, byte: u8) -> &mut LookMatcher {
        self.lineterm.0 = byte;
        self
    }
    pub fn get_line_terminator(&self) -> u8 {
        self.lineterm.0
    }
    #[inline]
    pub fn matches(&self, look: Look, haystack: &[u8], at: usize) -> bool {
        self.matches_inline(look, haystack, at)
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn matches_inline(&self, look: Look, haystack: &[u8], at: usize) -> bool {
        match look {
            Look::Start => self.is_start(haystack, at),
            Look::End => self.is_end(haystack, at),
            Look::StartLF => self.is_start_lf(haystack, at),
            Look::EndLF => self.is_end_lf(haystack, at),
            Look::StartCRLF => self.is_start_crlf(haystack, at),
            Look::EndCRLF => self.is_end_crlf(haystack, at),
            Look::WordAscii => self.is_word_ascii(haystack, at),
            Look::WordAsciiNegate => self.is_word_ascii_negate(haystack, at),
            Look::WordUnicode => self.is_word_unicode(haystack, at).unwrap(),
            Look::WordUnicodeNegate => self.is_word_unicode_negate(haystack, at).unwrap(),
            Look::WordStartAscii => self.is_word_start_ascii(haystack, at),
            Look::WordEndAscii => self.is_word_end_ascii(haystack, at),
            Look::WordStartUnicode => self.is_word_start_unicode(haystack, at).unwrap(),
            Look::WordEndUnicode => self.is_word_end_unicode(haystack, at).unwrap(),
            Look::WordStartHalfAscii => self.is_word_start_half_ascii(haystack, at),
            Look::WordEndHalfAscii => self.is_word_end_half_ascii(haystack, at),
            Look::WordStartHalfUnicode => self.is_word_start_half_unicode(haystack, at).unwrap(),
            Look::WordEndHalfUnicode => self.is_word_end_half_unicode(haystack, at).unwrap(),
        }
    }

    #[cfg_attr(all(), inline(always))]
    pub(crate) fn matches_set_inline(&self, set: LookSet, haystack: &[u8], at: usize) -> bool {
        if set.contains(Look::Start) {
            if !self.is_start(haystack, at) {
                return false;
            }
        }
        if set.contains(Look::End) {
            if !self.is_end(haystack, at) {
                return false;
            }
        }
        if set.contains(Look::StartLF) {
            if !self.is_start_lf(haystack, at) {
                return false;
            }
        }
        if set.contains(Look::EndLF) {
            if !self.is_end_lf(haystack, at) {
                return false;
            }
        }
        if set.contains(Look::StartCRLF) {
            if !self.is_start_crlf(haystack, at) {
                return false;
            }
        }
        if set.contains(Look::EndCRLF) {
            if !self.is_end_crlf(haystack, at) {
                return false;
            }
        }
        if set.contains(Look::WordAscii) {
            if !self.is_word_ascii(haystack, at) {
                return false;
            }
        }
        if set.contains(Look::WordAsciiNegate) {
            if !self.is_word_ascii_negate(haystack, at) {
                return false;
            }
        }
        if set.contains(Look::WordUnicode) {
            if !self.is_word_unicode(haystack, at).unwrap() {
                return false;
            }
        }
        if set.contains(Look::WordUnicodeNegate) {
            if !self.is_word_unicode_negate(haystack, at).unwrap() {
                return false;
            }
        }
        if set.contains(Look::WordStartAscii) {
            if !self.is_word_start_ascii(haystack, at) {
                return false;
            }
        }
        if set.contains(Look::WordEndAscii) {
            if !self.is_word_end_ascii(haystack, at) {
                return false;
            }
        }
        if set.contains(Look::WordStartUnicode) {
            if !self.is_word_start_unicode(haystack, at).unwrap() {
                return false;
            }
        }
        if set.contains(Look::WordEndUnicode) {
            if !self.is_word_end_unicode(haystack, at).unwrap() {
                return false;
            }
        }
        if set.contains(Look::WordStartHalfAscii) {
            if !self.is_word_start_half_ascii(haystack, at) {
                return false;
            }
        }
        if set.contains(Look::WordEndHalfAscii) {
            if !self.is_word_end_half_ascii(haystack, at) {
                return false;
            }
        }
        if set.contains(Look::WordStartHalfUnicode) {
            if !self.is_word_start_half_unicode(haystack, at).unwrap() {
                return false;
            }
        }
        if set.contains(Look::WordEndHalfUnicode) {
            if !self.is_word_end_half_unicode(haystack, at).unwrap() {
                return false;
            }
        }
        true
    }
    pub(crate) fn add_to_byteset(
        &self,
        look: Look,
        set: &mut crate::regex_engine::automata::util::alphabet::ByteClassSet,
    ) {
        match look {
            Look::Start | Look::End => {}
            Look::StartLF | Look::EndLF => {
                set.set_range(self.lineterm.0, self.lineterm.0);
            }
            Look::StartCRLF | Look::EndCRLF => {
                set.set_range(b'\r', b'\r');
                set.set_range(b'\n', b'\n');
            }
            Look::WordAscii
            | Look::WordAsciiNegate
            | Look::WordUnicode
            | Look::WordUnicodeNegate
            | Look::WordStartAscii
            | Look::WordEndAscii
            | Look::WordStartUnicode
            | Look::WordEndUnicode
            | Look::WordStartHalfAscii
            | Look::WordEndHalfAscii
            | Look::WordStartHalfUnicode
            | Look::WordEndHalfUnicode => {
                let iswb = utf8::is_word_byte;
                let asu8 = |b: u16| u8::try_from(b).unwrap();
                let mut b1: u16 = 0;
                let mut b2: u16;
                while b1 <= 255 {
                    b2 = b1 + 1;
                    while b2 <= 255 && iswb(asu8(b1)) == iswb(asu8(b2)) {
                        b2 += 1;
                    }
                    assert!(b2 <= 256);
                    set.set_range(asu8(b1), asu8(b2.checked_sub(1).unwrap()));
                    b1 = b2;
                }
            }
        }
    }
    #[inline]
    pub fn is_start(&self, _haystack: &[u8], at: usize) -> bool {
        at == 0
    }
    #[inline]
    pub fn is_end(&self, haystack: &[u8], at: usize) -> bool {
        at == haystack.len()
    }
    #[inline]
    pub fn is_start_lf(&self, haystack: &[u8], at: usize) -> bool {
        self.is_start(haystack, at) || haystack[at - 1] == self.lineterm.0
    }
    #[inline]
    pub fn is_end_lf(&self, haystack: &[u8], at: usize) -> bool {
        self.is_end(haystack, at) || haystack[at] == self.lineterm.0
    }
    #[inline]
    pub fn is_start_crlf(&self, haystack: &[u8], at: usize) -> bool {
        self.is_start(haystack, at)
            || haystack[at - 1] == b'\n'
            || (haystack[at - 1] == b'\r' && (at >= haystack.len() || haystack[at] != b'\n'))
    }
    #[inline]
    pub fn is_end_crlf(&self, haystack: &[u8], at: usize) -> bool {
        self.is_end(haystack, at)
            || haystack[at] == b'\r'
            || (haystack[at] == b'\n' && (at == 0 || haystack[at - 1] != b'\r'))
    }
    #[inline]
    pub fn is_word_ascii(&self, haystack: &[u8], at: usize) -> bool {
        let word_before = at > 0 && utf8::is_word_byte(haystack[at - 1]);
        let word_after = at < haystack.len() && utf8::is_word_byte(haystack[at]);
        word_before != word_after
    }
    #[inline]
    pub fn is_word_ascii_negate(&self, haystack: &[u8], at: usize) -> bool {
        !self.is_word_ascii(haystack, at)
    }
    #[inline]
    pub fn is_word_unicode(
        &self,
        haystack: &[u8],
        at: usize,
    ) -> Result<bool, UnicodeWordBoundaryError> {
        let word_before = is_word_char::rev(haystack, at)?;
        let word_after = is_word_char::fwd(haystack, at)?;
        Ok(word_before != word_after)
    }
    #[inline]
    pub fn is_word_unicode_negate(
        &self,
        haystack: &[u8],
        at: usize,
    ) -> Result<bool, UnicodeWordBoundaryError> {
        let word_before = at > 0
            && match utf8::decode_last(&haystack[..at]) {
                None | Some(Err(_)) => return Ok(false),
                Some(Ok(_)) => is_word_char::rev(haystack, at)?,
            };
        let word_after = at < haystack.len()
            && match utf8::decode(&haystack[at..]) {
                None | Some(Err(_)) => return Ok(false),
                Some(Ok(_)) => is_word_char::fwd(haystack, at)?,
            };
        Ok(word_before == word_after)
    }
    #[inline]
    pub fn is_word_start_ascii(&self, haystack: &[u8], at: usize) -> bool {
        let word_before = at > 0 && utf8::is_word_byte(haystack[at - 1]);
        let word_after = at < haystack.len() && utf8::is_word_byte(haystack[at]);
        !word_before && word_after
    }
    #[inline]
    pub fn is_word_end_ascii(&self, haystack: &[u8], at: usize) -> bool {
        let word_before = at > 0 && utf8::is_word_byte(haystack[at - 1]);
        let word_after = at < haystack.len() && utf8::is_word_byte(haystack[at]);
        word_before && !word_after
    }
    #[inline]
    pub fn is_word_start_unicode(
        &self,
        haystack: &[u8],
        at: usize,
    ) -> Result<bool, UnicodeWordBoundaryError> {
        let word_before = is_word_char::rev(haystack, at)?;
        let word_after = is_word_char::fwd(haystack, at)?;
        Ok(!word_before && word_after)
    }
    #[inline]
    pub fn is_word_end_unicode(
        &self,
        haystack: &[u8],
        at: usize,
    ) -> Result<bool, UnicodeWordBoundaryError> {
        let word_before = is_word_char::rev(haystack, at)?;
        let word_after = is_word_char::fwd(haystack, at)?;
        Ok(word_before && !word_after)
    }
    #[inline]
    pub fn is_word_start_half_ascii(&self, haystack: &[u8], at: usize) -> bool {
        let word_before = at > 0 && utf8::is_word_byte(haystack[at - 1]);
        !word_before
    }
    #[inline]
    pub fn is_word_end_half_ascii(&self, haystack: &[u8], at: usize) -> bool {
        let word_after = at < haystack.len() && utf8::is_word_byte(haystack[at]);
        !word_after
    }
    #[inline]
    pub fn is_word_start_half_unicode(
        &self,
        haystack: &[u8],
        at: usize,
    ) -> Result<bool, UnicodeWordBoundaryError> {
        let word_before = at > 0
            && match utf8::decode_last(&haystack[..at]) {
                None | Some(Err(_)) => return Ok(false),
                Some(Ok(_)) => is_word_char::rev(haystack, at)?,
            };
        Ok(!word_before)
    }
    #[inline]
    pub fn is_word_end_half_unicode(
        &self,
        haystack: &[u8],
        at: usize,
    ) -> Result<bool, UnicodeWordBoundaryError> {
        let word_after = at < haystack.len()
            && match utf8::decode(&haystack[at..]) {
                None | Some(Err(_)) => return Ok(false),
                Some(Ok(_)) => is_word_char::fwd(haystack, at)?,
            };
        Ok(!word_after)
    }
}
impl Default for LookMatcher {
    fn default() -> LookMatcher {
        LookMatcher::new()
    }
}
#[derive(Clone, Debug)]
pub struct UnicodeWordBoundaryError(());
impl UnicodeWordBoundaryError {
    pub fn check() -> Result<(), UnicodeWordBoundaryError> {
        is_word_char::check()
    }
}
impl std::error::Error for UnicodeWordBoundaryError {}
impl core::fmt::Display for UnicodeWordBoundaryError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Unicode-aware \\b and \\B are unavailable because the \
             requisite data tables are missing, please enable the \
             unicode-word-boundary feature"
        )
    }
}
mod is_word_char {
    use crate::regex_engine::automata::util::utf8;
    pub(super) fn check() -> Result<(), super::UnicodeWordBoundaryError> {
        Ok(())
    }
    #[cfg_attr(all(), inline(always))]
    pub(super) fn fwd(haystack: &[u8], at: usize) -> Result<bool, super::UnicodeWordBoundaryError> {
        Ok(match utf8::decode(&haystack[at..]) {
            None | Some(Err(_)) => false,
            Some(Ok(ch)) => is_word_character(ch),
        })
    }
    #[cfg_attr(all(), inline(always))]
    pub(super) fn rev(haystack: &[u8], at: usize) -> Result<bool, super::UnicodeWordBoundaryError> {
        Ok(match utf8::decode_last(&haystack[..at]) {
            None | Some(Err(_)) => false,
            Some(Ok(ch)) => is_word_character(ch),
        })
    }
    #[cfg_attr(all(), inline(always))]
    fn is_word_character(ch: char) -> bool {
        crate::regex_engine::syntax::try_is_word_character(ch).expect(
            "since unicode-word-boundary, syntax and unicode-perl \
             are all enabled, it is expected that \
             try_is_word_character succeeds",
        )
    }
}
