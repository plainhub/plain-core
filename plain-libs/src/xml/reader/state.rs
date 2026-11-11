use crate::xml::errors::{Error, IllFormedError, Result};
use crate::xml::events::{BytesCData, BytesDecl, BytesEnd, BytesPI, BytesStart, BytesText, Event};
use crate::xml::parser::{Parser, PiParser};
use crate::xml::reader::{BangType, Config, DtdParser, ParseState};
use crate::xml::utils::{is_whitespace, name_len};
use std::borrow::Cow;
use std::fmt::Debug;
#[derive(Clone, Debug)]
pub(super) struct ReaderState {
    pub offset: u64,
    pub last_error_offset: u64,
    pub state: ParseState,
    pub config: Config,
    opened_buffer: String,
    opened_starts: Vec<usize>,
}
impl ReaderState {
    pub fn emit_text<'b>(&mut self, buf: &'b str) -> Result<BytesText<'b>> {
        let content = if self.config.trim_text_end {
            let len = buf
                .as_bytes()
                .iter()
                .rposition(|&b| !is_whitespace(b))
                .map_or(0, |p| p + 1);
            &buf[..len]
        } else {
            buf
        };
        Ok(BytesText::wrap(content))
    }
    pub fn emit_bang<'b>(&mut self, bang_type: BangType, buf: &'b str) -> Result<Event<'b>> {
        let buf_bytes = buf.as_bytes();
        debug_assert!(
            buf_bytes.starts_with(b"<!"),
            "CDATA, comment or DOCTYPE must start from '<!':\n{:?}",
            crate::xml::utils::Bytes(buf_bytes)
        );
        debug_assert!(
            buf_bytes.ends_with(b">"),
            "CDATA, comment or DOCTYPE must end with '>':\n{:?}",
            crate::xml::utils::Bytes(buf_bytes)
        );
        let uncased_starts_with = |string: &[u8], prefix: &[u8]| {
            string.len() >= prefix.len() && string[..prefix.len()].eq_ignore_ascii_case(prefix)
        };
        let len = buf.len();
        match bang_type {
            BangType::Comment if buf_bytes.starts_with(b"<!--") => {
                debug_assert!(
                    buf_bytes.ends_with(b"-->"),
                    "comment must end with '-->':\n{:?}",
                    crate::xml::utils::Bytes(buf_bytes)
                );
                if self.config.check_comments {
                    let mut haystack = &buf_bytes[4..len - 3];
                    let mut off = 0;
                    while let Some(p) = memchr::memchr(b'-', haystack) {
                        off += p + 1;
                        if buf_bytes[4 + off] == b'-' {
                            self.last_error_offset = self.offset - len as u64 + 4 + p as u64;
                            return Err(Error::IllFormed(IllFormedError::DoubleHyphenInComment));
                        }
                        haystack = &haystack[p + 1..];
                    }
                }
                Ok(Event::Comment(BytesText::wrap(&buf[4..len - 3])))
            }
            BangType::CData if buf_bytes.starts_with(b"<![CDATA[") => {
                debug_assert!(
                    buf_bytes.ends_with(b"]]>"),
                    "CDATA must end with ']]>':\n{:?}",
                    crate::xml::utils::Bytes(buf_bytes)
                );
                Ok(Event::CData(BytesCData::wrap(&buf[9..len - 3])))
            }
            BangType::DocType(DtdParser::Finished)
                if uncased_starts_with(buf_bytes, b"<!DOCTYPE") =>
            {
                match buf_bytes[9..len - 1]
                    .iter()
                    .position(|&b| !is_whitespace(b))
                {
                    Some(start) => Ok(Event::DocType(BytesText::wrap(&buf[9 + start..len - 1]))),
                    None => {
                        self.last_error_offset = self.offset - 1;
                        Err(Error::IllFormed(IllFormedError::MissingDoctypeName))
                    }
                }
            }
            _ => {
                self.last_error_offset = self.offset - len as u64;
                Err(Error::Syntax(bang_type.to_err()))
            }
        }
    }
    pub fn emit_end<'b>(&mut self, buf: &'b str) -> Result<Event<'b>> {
        let buf_bytes = buf.as_bytes();
        debug_assert!(
            buf_bytes.starts_with(b"</"),
            "end tag must start from '</':\n{:?}",
            crate::xml::utils::Bytes(buf_bytes)
        );
        debug_assert!(
            buf_bytes.ends_with(b">"),
            "end tag must end with '>':\n{:?}",
            crate::xml::utils::Bytes(buf_bytes)
        );
        let content = &buf[2..buf.len() - 1];
        let name = if self.config.trim_markup_names_in_closing_tags {
            if let Some(pos_end_name) = content.as_bytes().iter().rposition(|&b| !is_whitespace(b))
            {
                &content[..pos_end_name + 1]
            } else {
                content
            }
        } else {
            content
        };
        match self.opened_starts.pop() {
            Some(start) => {
                if self.config.check_end_names {
                    let expected = &self.opened_buffer[start..];
                    if name != expected {
                        let expected = expected.to_owned();
                        self.opened_buffer.truncate(start);
                        self.last_error_offset = self.offset - buf.len() as u64;
                        return Err(Error::IllFormed(IllFormedError::MismatchedEndTag {
                            expected,
                            found: name.to_owned(),
                        }));
                    }
                }
                self.opened_buffer.truncate(start);
            }
            None => {
                if !self.config.allow_unmatched_ends {
                    self.last_error_offset = self.offset - buf.len() as u64;
                    return Err(Error::IllFormed(IllFormedError::UnmatchedEndTag(
                        name.to_owned(),
                    )));
                }
            }
        }
        Ok(Event::End(BytesEnd::wrap(Cow::Borrowed(name))))
    }
    pub fn emit_question_mark<'b>(&mut self, buf: &'b str) -> Result<Event<'b>> {
        let buf_bytes = buf.as_bytes();
        debug_assert!(
            buf_bytes.starts_with(b"<?"),
            "processing instruction or XML declaration must start from '<?':\n{:?}",
            crate::xml::utils::Bytes(buf_bytes)
        );
        debug_assert!(
            buf_bytes.ends_with(b"?>"),
            "processing instruction or XML declaration must end with '?>':\n{:?}",
            crate::xml::utils::Bytes(buf_bytes)
        );
        let len = buf.len();
        if len > 3 {
            let content = &buf[2..len - 2];
            let content_bytes = content.as_bytes();
            let len = content.len();
            if content_bytes.starts_with(b"xml") && (len == 3 || is_whitespace(content_bytes[3])) {
                let event = BytesDecl::from_start(BytesStart::wrap(content, 3));
                Ok(Event::Decl(event))
            } else {
                Ok(Event::PI(BytesPI::wrap(content, name_len(content_bytes))))
            }
        } else {
            self.last_error_offset = self.offset - len as u64;
            Err(Error::Syntax(PiParser(false).eof_error(buf_bytes)))
        }
    }
    pub fn emit_start<'b>(&mut self, buf: &'b str) -> Result<Event<'b>> {
        let buf_bytes = buf.as_bytes();
        debug_assert!(
            buf_bytes.starts_with(b"<"),
            "start or empty tag must start from '<':\n{:?}",
            crate::xml::utils::Bytes(buf_bytes)
        );
        debug_assert!(
            buf_bytes.ends_with(b">"),
            "start or empty tag must end with '>':\n{:?}",
            crate::xml::utils::Bytes(buf_bytes)
        );
        let content = &buf[1..];
        let content_bytes = &buf_bytes[1..];
        if let Some(content_bytes) = content_bytes.strip_suffix(b"/>") {
            let content = &content[..content_bytes.len()];
            let event = BytesStart::wrap(content, name_len(content_bytes));
            if self.config.expand_empty_elements {
                self.state = ParseState::InsideEmpty;
                self.opened_starts.push(self.opened_buffer.len());
                self.opened_buffer.push_str(event.name().as_ref());
                Ok(Event::Start(event))
            } else {
                Ok(Event::Empty(event))
            }
        } else {
            let content_bytes = &content_bytes[..content_bytes.len() - 1];
            let content = &content[..content_bytes.len()];
            let event = BytesStart::wrap(content, name_len(content_bytes));
            self.opened_starts.push(self.opened_buffer.len());
            self.opened_buffer.push_str(event.name().as_ref());
            Ok(Event::Start(event))
        }
    }
    #[inline]
    pub fn close_expanded_empty(&mut self) -> BytesEnd<'static> {
        self.state = ParseState::InsideText;
        let name = self
            .opened_buffer
            .split_off(self.opened_starts.pop().unwrap());
        BytesEnd::wrap(Cow::Owned(name))
    }
}
impl Default for ReaderState {
    fn default() -> Self {
        Self {
            offset: 0,
            last_error_offset: 0,
            state: ParseState::Init,
            config: Config::default(),
            opened_buffer: String::new(),
            opened_starts: Vec::new(),
        }
    }
}
