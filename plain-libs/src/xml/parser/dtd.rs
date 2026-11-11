use crate::xml::parser::{CommentParser, ElementParser, Parser, PiParser};
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DtdParser {
    BeforeInternalSubset(u8),
    InsideOfInternalSubset,
    AfterInternalSubset,
    InComment(CommentParser),
    InPi(PiParser),
    InElementDecl,
    InQuoteSensitive(ElementParser),
    UndecidedMarkup(usize),
    Finished,
}
impl DtdParser {
    pub fn feed(&mut self, buf: &[u8], chunk: &[u8]) -> Option<usize> {
        let mut cur = chunk;
        while !cur.is_empty() {
            match *self {
                Self::BeforeInternalSubset(0) => {
                    if let Some(i) = cur
                        .iter()
                        .position(|&b| matches!(b, b'\'' | b'"' | b'[' | b'>'))
                    {
                        let b = cur[i];
                        match b {
                            b'\'' | b'"' => {
                                *self = Self::BeforeInternalSubset(b);
                                cur = &cur[i + 1..];
                                continue;
                            }
                            b'[' => {
                                *self = Self::InsideOfInternalSubset;
                                cur = &cur[i + 1..];
                                continue;
                            }
                            b'>' => {
                                *self = Self::Finished;
                                return Some(chunk.len() - cur.len() + i);
                            }
                            _ => {}
                        }
                        continue;
                    }
                    break;
                }
                Self::BeforeInternalSubset(quote) => {
                    if let Some(i) = memchr::memchr(quote, cur) {
                        *self = Self::BeforeInternalSubset(0);
                        cur = &cur[i + 1..];
                        continue;
                    }
                    break;
                }
                Self::InsideOfInternalSubset => {
                    if let Some(i) = memchr::memchr2(b']', b'<', cur) {
                        if cur[i] == b']' {
                            *self = Self::AfterInternalSubset;
                            cur = &cur[i + 1..];
                            continue;
                        }
                        if let Some(skip) = self.switch(&cur[i + 1..]) {
                            cur = &cur[i + 1 + skip..];
                            continue;
                        }
                        let skipped = cur.len() - i - 1;
                        if skipped >= 9 {
                            cur = &cur[i + 1..];
                            *self = Self::InElementDecl;
                            continue;
                        }
                        *self = Self::UndecidedMarkup(skipped);
                    }
                    break;
                }
                Self::AfterInternalSubset => {
                    if let Some(i) = memchr::memchr(b'>', cur) {
                        *self = Self::Finished;
                        return Some(chunk.len() - cur.len() + i);
                    }
                    break;
                }
                Self::InComment(ref mut parser) => {
                    if let Some(i) = parser.feed(cur) {
                        *self = Self::InsideOfInternalSubset;
                        cur = &cur[i..];
                        continue;
                    }
                    break;
                }
                Self::InPi(ref mut parser) => {
                    if let Some(i) = parser.feed(cur) {
                        *self = Self::InsideOfInternalSubset;
                        cur = &cur[i..];
                        continue;
                    }
                    break;
                }
                Self::InElementDecl => {
                    if let Some(i) = memchr::memchr(b'>', cur) {
                        *self = Self::InsideOfInternalSubset;
                        cur = &cur[i + 1..];
                        continue;
                    }
                    break;
                }
                Self::InQuoteSensitive(ref mut parser) => {
                    if let Some(i) = parser.feed(cur) {
                        *self = Self::InsideOfInternalSubset;
                        cur = &cur[i..];
                        continue;
                    }
                    break;
                }
                Self::UndecidedMarkup(skipped) => {
                    let mut bytes = [0u8; 9];
                    bytes[..skipped].copy_from_slice(&buf[buf.len() - skipped..]);
                    let end = bytes.len().min(skipped + cur.len());
                    bytes[skipped..end].copy_from_slice(&cur[..end - skipped]);
                    if let Some(skip) = self.switch(&bytes[..end]) {
                        cur = &cur[skip - skipped..];
                        continue;
                    }
                    if end == bytes.len() {
                        cur = &cur[end - skipped..];
                        *self = Self::InElementDecl;
                        continue;
                    }
                    *self = Self::UndecidedMarkup(skipped + cur.len());
                    break;
                }
                Self::Finished => break,
            }
        }
        None
    }
    #[inline]
    fn switch(&mut self, markup: &[u8]) -> Option<usize> {
        match markup {
            [b'?', ..] => {
                *self = Self::InPi(PiParser(false));
                Some(1)
            }
            [b'!', b'-', b'-', ..] => {
                *self = Self::InComment(CommentParser::Seen0);
                Some(3)
            }
            [b'!', b'E', b'L', b'E', b'M', b'E', b'N', b'T', ..] => {
                *self = Self::InElementDecl;
                Some(8)
            }
            [b'!', b'E', b'N', b'T', b'I', b'T', b'Y', ..] => {
                *self = Self::InQuoteSensitive(ElementParser::Outside);
                Some(7)
            }
            [b'!', b'A', b'T', b'T', b'L', b'I', b'S', b'T', ..] => {
                *self = Self::InQuoteSensitive(ElementParser::Outside);
                Some(8)
            }
            [b'!', b'N', b'O', b'T', b'A', b'T', b'I', b'O', b'N', ..] => {
                *self = Self::InQuoteSensitive(ElementParser::Outside);
                Some(9)
            }
            _ => {
                if let Some(i) = memchr::memchr(b'>', markup) {
                    *self = Self::InsideOfInternalSubset;
                    Some(i + 1)
                } else {
                    None
                }
            }
        }
    }
}
