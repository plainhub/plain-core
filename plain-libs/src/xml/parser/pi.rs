use crate::xml::errors::SyntaxError;
use crate::xml::parser::Parser;
use crate::xml::utils::is_whitespace;
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PiParser(
    #[doc = " A flag that indicates was the `bytes` in the previous attempt to find the"]
    #[doc = " end ended with `?`."]
    pub bool,
);
impl Parser for PiParser {
    #[inline]
    fn feed(&mut self, bytes: &[u8]) -> Option<usize> {
        for i in memchr::memchr_iter(b'>', bytes) {
            match i {
                0 if self.0 => return Some(0),
                i if i > 0 && bytes[i - 1] == b'?' => return Some(i),
                _ => {}
            }
        }
        self.0 = bytes.last().copied() == Some(b'?');
        None
    }
    #[inline]
    fn eof_error(self, content: &[u8]) -> SyntaxError {
        let is_xml_decl = content.starts_with(b"<?xml")
            && content
                .get(5)
                .map_or(true, |&b| is_whitespace(b) || b == b'?');
        if is_xml_decl {
            SyntaxError::UnclosedXmlDecl
        } else {
            SyntaxError::UnclosedPI
        }
    }
}
