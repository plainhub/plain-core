use crate::xml::errors::SyntaxError;
use crate::xml::parser::Parser;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommentParser {
    Seen0,
    Seen1,
    Seen2,
}
impl Default for CommentParser {
    #[inline]
    fn default() -> Self {
        Self::Seen0
    }
}
impl Parser for CommentParser {
    #[inline]
    fn feed(&mut self, bytes: &[u8]) -> Option<usize> {
        let result = match self {
            Self::Seen0 => seen0(bytes),
            Self::Seen1 => seen1(bytes),
            Self::Seen2 => seen2(bytes),
        };
        if let Some(r) = result {
            return Some(r);
        }
        if bytes.ends_with(b"--") {
            *self = Self::Seen2;
        } else {
            self.next_state(bytes.last().copied());
        }
        None
    }
    #[inline]
    fn eof_error(self, _content: &[u8]) -> SyntaxError {
        SyntaxError::UnclosedComment
    }
}
impl CommentParser {
    #[inline]
    fn next_state(&mut self, last: Option<u8>) {
        match (*self, last) {
            (Self::Seen0, Some(b'-')) => *self = Self::Seen1,
            (Self::Seen1, Some(b'-')) => *self = Self::Seen2,
            (Self::Seen1, Some(_)) => *self = Self::Seen0,
            (Self::Seen2, Some(b'-')) => {}
            (Self::Seen2, Some(_)) => *self = Self::Seen0,
            _ => {}
        }
    }
}
#[inline]
fn seen0(bytes: &[u8]) -> Option<usize> {
    for i in crate::memchr::memchr_iter(b'>', bytes) {
        if bytes[..i].ends_with(b"--") {
            return Some(i + 1);
        }
    }
    None
}
#[inline]
fn seen1(bytes: &[u8]) -> Option<usize> {
    if bytes.starts_with(b"->") {
        return Some(2);
    }
    seen0(bytes)
}
#[inline]
fn seen2(bytes: &[u8]) -> Option<usize> {
    match bytes.first() {
        None => None,
        Some(b'>') => Some(1),
        Some(_) => seen1(bytes),
    }
}
