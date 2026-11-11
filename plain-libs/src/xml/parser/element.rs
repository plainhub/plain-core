use crate::xml::errors::SyntaxError;
use crate::xml::parser::Parser;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ElementParser {
    Outside,
    SingleQ,
    DoubleQ,
}
impl Parser for ElementParser {
    #[inline]
    fn feed(&mut self, bytes: &[u8]) -> Option<usize> {
        for i in memchr::memchr3_iter(b'>', b'\'', b'"', bytes) {
            *self = match (*self, bytes[i]) {
                (Self::Outside, b'>') => return Some(i),
                (Self::Outside, b'\'') => Self::SingleQ,
                (Self::Outside, b'\"') => Self::DoubleQ,
                (Self::SingleQ, b'\'') | (Self::DoubleQ, b'"') => Self::Outside,
                _ => continue,
            };
        }
        None
    }
    #[inline]
    fn eof_error(self, _content: &[u8]) -> SyntaxError {
        match self {
            Self::Outside => SyntaxError::UnclosedTag,
            Self::SingleQ => SyntaxError::UnclosedSingleQuotedAttributeValue,
            Self::DoubleQ => SyntaxError::UnclosedDoubleQuotedAttributeValue,
        }
    }
}
impl Default for ElementParser {
    #[inline]
    fn default() -> Self {
        Self::Outside
    }
}
