use crate::xml::errors::{Error, Result};
use crate::xml::events::{BytesText, Event};
use crate::xml::name::QName;
use crate::xml::parser::Parser;
use crate::xml::reader::{BangType, ReadRefResult, ReadTextResult, Reader, Span, XmlSource};
use crate::xml::utils::is_whitespace;
use std::io;
impl<'a> Reader<&'a [u8]> {
    pub fn from_str(s: &'a str) -> Self {
        Self::from_reader(s.as_bytes())
    }
    #[inline]
    pub fn read_event(&mut self) -> Result<Event<'a>> {
        self.read_event_impl(())
    }
    pub fn read_to_end(&mut self, end: QName) -> Result<Span> {
        Ok(read_to_end!(self, end, (), read_event_impl, {}))
    }
    pub fn read_text(&mut self, end: QName) -> Result<BytesText<'a>> {
        let buffer = self.reader;
        let span = self.read_to_end(end)?;
        let len = span.end - span.start;
        let text = std::str::from_utf8(&buffer[0..len as usize])?;
        Ok(BytesText::wrap(text))
    }
}
impl<'a> XmlSource<'a, ()> for &'a [u8] {
    #[inline]
    fn remove_utf8_bom(&mut self) -> io::Result<()> {
        if self.starts_with(crate::xml::encoding::UTF8_BOM) {
            *self = &self[crate::xml::encoding::UTF8_BOM.len()..];
        }
        Ok(())
    }
    #[inline]
    fn read_text(&mut self, _buf: (), position: &mut u64) -> ReadTextResult<'a, ()> {
        match memchr::memchr2(b'<', b'&', self) {
            Some(0) if self[0] == b'<' => ReadTextResult::Markup(()),
            Some(0) => ReadTextResult::Ref(()),
            Some(i) if self[i] == b'<' => {
                let (bytes, rest) = self.split_at(i);
                *self = rest;
                *position += i as u64;
                match std::str::from_utf8(bytes) {
                    Ok(s) => ReadTextResult::UpToMarkup(s),
                    Err(e) => ReadTextResult::Err(e.into()),
                }
            }
            Some(i) => {
                let (bytes, rest) = self.split_at(i);
                *self = rest;
                *position += i as u64;
                match std::str::from_utf8(bytes) {
                    Ok(s) => ReadTextResult::UpToRef(s),
                    Err(e) => ReadTextResult::Err(e.into()),
                }
            }
            None => {
                let bytes = &self[..];
                *self = &[];
                *position += bytes.len() as u64;
                match std::str::from_utf8(bytes) {
                    Ok(s) => ReadTextResult::UpToEof(s),
                    Err(e) => ReadTextResult::Err(e.into()),
                }
            }
        }
    }
    #[inline]
    fn read_ref(&mut self, _buf: (), position: &mut u64) -> ReadRefResult<'a> {
        debug_assert!(
            self.starts_with(b"&"),
            "`read_ref` must be called at `&`:\n{:?}",
            crate::xml::utils::Bytes(self)
        );
        match memchr::memchr3(b';', b'&', b'<', &self[1..]) {
            Some(i) if self[i + 1] == b';' => {
                let end = i + 2;
                let (bytes, rest) = self.split_at(end);
                *self = rest;
                *position += end as u64;
                match std::str::from_utf8(bytes) {
                    Ok(s) => ReadRefResult::Ref(s),
                    Err(e) => ReadRefResult::Err(e.into()),
                }
            }
            Some(i) => {
                let is_amp = self[i + 1] == b'&';
                let (bytes, rest) = self.split_at(i + 1);
                *self = rest;
                *position += i as u64 + 1;
                match std::str::from_utf8(bytes) {
                    Ok(s) => {
                        if is_amp {
                            ReadRefResult::UpToRef(s)
                        } else {
                            ReadRefResult::UpToMarkup(s)
                        }
                    }
                    Err(e) => ReadRefResult::Err(e.into()),
                }
            }
            None => {
                let bytes = &self[..];
                *self = &[];
                *position += bytes.len() as u64;
                match std::str::from_utf8(bytes) {
                    Ok(s) => ReadRefResult::UpToEof(s),
                    Err(e) => ReadRefResult::Err(e.into()),
                }
            }
        }
    }
    #[inline]
    fn read_with<P>(&mut self, mut parser: P, _buf: (), position: &mut u64) -> Result<&'a str>
    where
        P: Parser,
    {
        if let Some(i) = parser.feed(self) {
            let used = i + 1;
            *position += used as u64;
            let (bytes, rest) = self.split_at(used);
            *self = rest;
            return Ok(std::str::from_utf8(bytes)?);
        }
        *position += self.len() as u64;
        Err(Error::Syntax(parser.eof_error(self)))
    }
    #[inline]
    fn read_bang_element(&mut self, _buf: (), position: &mut u64) -> Result<(BangType, &'a str)> {
        debug_assert!(
            self.starts_with(b"<!"),
            "`read_bang_element` must be called at `<!`:\n{:?}",
            crate::xml::utils::Bytes(self)
        );
        let mut bang_type = BangType::new(self.get(2).copied())?;
        if let Some(i) = bang_type.feed(&[], self) {
            let consumed = i + 1;
            *position += consumed as u64;
            let (bytes, rest) = self.split_at(consumed);
            *self = rest;
            return Ok((bang_type, std::str::from_utf8(bytes)?));
        }
        *position += self.len() as u64;
        Err(Error::Syntax(bang_type.to_err()))
    }
    #[inline]
    fn skip_whitespace(&mut self, position: &mut u64) -> io::Result<()> {
        let whitespaces = self
            .iter()
            .position(|b| !is_whitespace(*b))
            .unwrap_or(self.len());
        *position += whitespaces as u64;
        *self = &self[whitespaces..];
        Ok(())
    }
    #[inline]
    fn peek_one(&mut self) -> io::Result<Option<u8>> {
        debug_assert!(
            self.starts_with(b"<"),
            "markup must start from '<':\n{:?}",
            crate::xml::utils::Bytes(self)
        );
        Ok(self.get(1).copied())
    }
}
