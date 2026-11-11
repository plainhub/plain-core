use std::borrow::Cow;
use std::fmt::{self, Debug, Formatter};
use std::iter::FusedIterator;
pub fn write_cow_string(f: &mut Formatter, cow_string: &Cow<str>) -> fmt::Result {
    match cow_string {
        Cow::Owned(s) => write!(f, "Owned({s:?})"),
        Cow::Borrowed(s) => write!(f, "Borrowed({s:?})"),
    }
}
pub fn write_byte_string(f: &mut Formatter, byte_string: &[u8]) -> fmt::Result {
    write!(f, "\"")?;
    for b in byte_string {
        match *b {
            32..=33 | 35..=126 => write!(f, "{}", *b as char)?,
            34 => write!(f, "\\\"")?,
            _ => write!(f, "{:#02X}", b)?,
        }
    }
    write!(f, "\"")?;
    Ok(())
}
pub struct Bytes<'de>(pub &'de [u8]);
impl<'de> Debug for Bytes<'de> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write_byte_string(f, self.0)
    }
}
pub const fn is_whitespace(b: u8) -> bool {
    matches!(b, b' ' | b'\r' | b'\n' | b'\t')
}
#[inline]
pub const fn name_len(mut bytes: &[u8]) -> usize {
    let mut len = 0;
    while let [first, rest @ ..] = bytes {
        if is_whitespace(*first) {
            break;
        }
        len += 1;
        bytes = rest;
    }
    len
}
#[inline]
pub const fn trim_xml_start(s: &str) -> &str {
    let bytes = s.as_bytes();
    let mut pos = 0;
    while pos < bytes.len() {
        if !is_whitespace(bytes[pos]) {
            break;
        }
        pos += 1;
    }
    let (_, rest) = s.split_at(pos);
    rest
}
#[inline]
pub const fn trim_xml_end(s: &str) -> &str {
    let bytes = s.as_bytes();
    let mut end = bytes.len();
    while end > 0 {
        if !is_whitespace(bytes[end - 1]) {
            break;
        }
        end -= 1;
    }
    let (rest, _) = s.split_at(end);
    rest
}
#[inline]
pub const fn trim_xml_spaces(text: &str) -> &str {
    trim_xml_end(trim_xml_start(text))
}
#[derive(Debug, Clone)]
pub(crate) struct CDataIterator<'a> {
    unprocessed: &'a str,
    finished: bool,
}
impl<'a> CDataIterator<'a> {
    pub const fn new(value: &'a str) -> Self {
        Self {
            unprocessed: value,
            finished: false,
        }
    }
}
impl<'a> Iterator for CDataIterator<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<&'a str> {
        if self.finished {
            return None;
        }
        for gt in crate::memchr::memchr_iter(b'>', self.unprocessed.as_bytes()) {
            let (slice, rest) = self.unprocessed.split_at(gt);
            if slice.ends_with("]]") {
                self.unprocessed = rest;
                return Some(slice);
            }
        }
        self.finished = true;
        Some(self.unprocessed)
    }
}
impl FusedIterator for CDataIterator<'_> {}
