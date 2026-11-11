use crate::xml::encoding;
use crate::xml::errors::{Error, Result};
use crate::xml::events::{BytesText, Event};
use crate::xml::name::QName;
use crate::xml::parser::Parser;
use crate::xml::reader::{BangType, ReadRefResult, ReadTextResult, Reader, Span, XmlSource};
use crate::xml::utils::is_whitespace;
use std::io::{self, BufRead};
macro_rules! impl_buffered_source {
    () => {
        #[inline]
        fn remove_utf8_bom(&mut self) -> io::Result<()> {
            loop {
                break match self.fill_buf() {
                    Ok(n) => {
                        if n.starts_with(encoding::UTF8_BOM) {
                            self.consume(encoding::UTF8_BOM.len());
                        }
                        Ok(())
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => Err(e),
                };
            }
        }
        #[inline]
        fn read_text(
            &mut self,
            buf: &'b mut Vec<u8>,
            position: &mut u64,
        ) -> ReadTextResult<'b, &'b mut Vec<u8>> {
            let mut read = 0;
            let start = buf.len();
            loop {
                let available = match self.fill_buf() {
                    Ok(n) if n.is_empty() => break,
                    Ok(n) => n,
                    Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => {
                        *position += read;
                        return ReadTextResult::Err(e.into());
                    }
                };
                match crate::memchr::memchr2(b'<', b'&', available) {
                    Some(0) if read == 0 && available[0] == b'<' => {
                        return ReadTextResult::Markup(buf);
                    }
                    Some(0) if read == 0 => return ReadTextResult::Ref(buf),
                    Some(i) if available[i] == b'<' => {
                        buf.extend_from_slice(&available[..i]);
                        self.consume(i);
                        read += i as u64;
                        *position += read;
                        return match std::str::from_utf8(&buf[start..]) {
                            Ok(s) => ReadTextResult::UpToMarkup(s),
                            Err(e) => ReadTextResult::Err(e.into()),
                        };
                    }
                    Some(i) => {
                        buf.extend_from_slice(&available[..i]);
                        self.consume(i);
                        read += i as u64;
                        *position += read;
                        return match std::str::from_utf8(&buf[start..]) {
                            Ok(s) => ReadTextResult::UpToRef(s),
                            Err(e) => ReadTextResult::Err(e.into()),
                        };
                    }
                    None => {
                        buf.extend_from_slice(available);
                        let used = available.len();
                        self.consume(used);
                        read += used as u64;
                    }
                }
            }
            *position += read;
            match std::str::from_utf8(&buf[start..]) {
                Ok(s) => ReadTextResult::UpToEof(s),
                Err(e) => ReadTextResult::Err(e.into()),
            }
        }
        #[inline]
        fn read_ref(&mut self, buf: &'b mut Vec<u8>, position: &mut u64) -> ReadRefResult<'b> {
            let mut read = 0;
            let start = buf.len();
            loop {
                let available = match self.fill_buf() {
                    Ok(n) if n.is_empty() => break,
                    Ok(n) => n,
                    Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => {
                        *position += read;
                        return ReadRefResult::Err(e.into());
                    }
                };
                if read == 0 {
                    debug_assert!(
                        available.starts_with(b"&"),
                        "`read_ref` must be called at `&`:\n{:?}",
                        crate::xml::utils::Bytes(available)
                    );
                    buf.push(b'&');
                    self.consume(1);
                    read += 1;
                    continue;
                }
                match crate::memchr::memchr3(b';', b'&', b'<', available) {
                    Some(i) if available[i] == b';' => {
                        let used = i + 1;
                        buf.extend_from_slice(&available[..used]);
                        self.consume(used);
                        read += used as u64;
                        *position += read;
                        return match std::str::from_utf8(&buf[start..]) {
                            Ok(s) => ReadRefResult::Ref(s),
                            Err(e) => ReadRefResult::Err(e.into()),
                        };
                    }
                    Some(i) => {
                        let is_amp = available[i] == b'&';
                        buf.extend_from_slice(&available[..i]);
                        self.consume(i);
                        read += i as u64;
                        *position += read;
                        return match std::str::from_utf8(&buf[start..]) {
                            Ok(s) => {
                                if is_amp {
                                    ReadRefResult::UpToRef(s)
                                } else {
                                    ReadRefResult::UpToMarkup(s)
                                }
                            }
                            Err(e) => ReadRefResult::Err(e.into()),
                        };
                    }
                    None => {
                        buf.extend_from_slice(available);
                        let used = available.len();
                        self.consume(used);
                        read += used as u64;
                    }
                }
            }
            *position += read;
            match std::str::from_utf8(&buf[start..]) {
                Ok(s) => ReadRefResult::UpToEof(s),
                Err(e) => ReadRefResult::Err(e.into()),
            }
        }
        #[inline]
        fn read_with<P: Parser>(
            &mut self,
            mut parser: P,
            buf: &'b mut Vec<u8>,
            position: &mut u64,
        ) -> Result<&'b str> {
            let mut read = 1;
            let start = buf.len();
            buf.push(b'<');
            loop {
                let available = match self.fill_buf() {
                    Ok(n) if n.is_empty() => break,
                    Ok(n) => n,
                    Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => {
                        *position += read;
                        return Err(Error::from(e));
                    }
                };
                if let Some(i) = parser.feed(available) {
                    let used = i + 1;
                    buf.extend_from_slice(&available[..used]);
                    self.consume(used);
                    read += used as u64;
                    *position += read;
                    return Ok(std::str::from_utf8(&buf[start..])?);
                }
                buf.extend_from_slice(available);
                let used = available.len();
                self.consume(used);
                read += used as u64;
            }
            *position += read;
            Err(Error::Syntax(parser.eof_error(&buf[start..])))
        }
        #[inline]
        fn read_bang_element(
            &mut self,
            buf: &'b mut Vec<u8>,
            position: &mut u64,
        ) -> Result<(BangType, &'b str)> {
            let start = buf.len();
            let mut read = 2;
            buf.push(b'<');
            buf.push(b'!');
            self.consume(1);
            let mut bang_type = loop {
                break match self.fill_buf() {
                    Ok(n) => BangType::new(n.first().cloned())?,
                    Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(Error::from(e)),
                };
            };
            loop {
                let available = match self.fill_buf() {
                    Ok(n) if n.is_empty() => break,
                    Ok(n) => n,
                    Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => {
                        *position += read;
                        return Err(Error::from(e));
                    }
                };
                if let Some(i) = bang_type.feed(&buf[start..], available) {
                    let consumed = i + 1;
                    buf.extend_from_slice(&available[..consumed]);
                    self.consume(consumed);
                    read += consumed as u64;
                    *position += read;
                    return Ok((bang_type, std::str::from_utf8(&buf[start..])?));
                }
                buf.extend_from_slice(available);
                let used = available.len();
                self.consume(used);
                read += used as u64;
            }
            *position += read;
            Err(Error::Syntax(bang_type.to_err()))
        }
        #[inline]
        fn skip_whitespace(&mut self, position: &mut u64) -> io::Result<()> {
            loop {
                break match self.fill_buf() {
                    Ok(n) => {
                        let count = n.iter().position(|b| !is_whitespace(*b)).unwrap_or(n.len());
                        if count > 0 {
                            self.consume(count);
                            *position += count as u64;
                            continue;
                        } else {
                            Ok(())
                        }
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => Err(e),
                };
            }
        }
        #[inline]
        fn peek_one(&mut self) -> io::Result<Option<u8>> {
            self.consume(1);
            let available = loop {
                break match self.fill_buf() {
                    Ok(n) => n,
                    Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                };
            };
            Ok(available.first().cloned())
        }
    };
}
impl<'b, R: BufRead> XmlSource<'b, &'b mut Vec<u8>> for R {
    impl_buffered_source!();
}
impl<R: BufRead> Reader<R> {
    #[inline]
    pub fn read_event_into<'b>(&mut self, buf: &'b mut Vec<u8>) -> Result<Event<'b>> {
        self.read_event_impl(buf)
    }
    pub fn read_to_end_into(&mut self, end: QName, buf: &mut Vec<u8>) -> Result<Span> {
        Ok(read_to_end!(self, end, buf, read_event_impl, {
            buf.clear();
        }))
    }
    pub fn read_text_into<'b>(
        &mut self,
        end: QName,
        buf: &'b mut Vec<u8>,
    ) -> Result<BytesText<'b>> {
        let start = buf.len();
        let span = read_to_end!(self, end, buf, read_event_impl, {});
        let len = span.end - span.start;
        let end = start + len as usize;
        let text = std::str::from_utf8(&buf[start..end])?;
        Ok(BytesText::wrap(text))
    }
}
