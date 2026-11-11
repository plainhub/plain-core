use crate::compression::Crc;
use crate::compression::io::{BufRead, Error, ErrorKind, Read, Result};
use core::time;
use std::boxed::Box;
use std::vec::Vec;
pub static FHCRC: u8 = 1 << 1;
pub static FEXTRA: u8 = 1 << 2;
pub static FNAME: u8 = 1 << 3;
pub static FCOMMENT: u8 = 1 << 4;
pub static FRESERVED: u8 = 1 << 5 | 1 << 6 | 1 << 7;
pub mod bufread;
pub mod read;
const MAX_HEADER_BUF: usize = 65535;
#[derive(PartialEq, Clone, Debug, Default)]
pub struct GzHeader {
    extra: Option<Vec<u8>>,
    filename: Option<Vec<u8>>,
    comment: Option<Vec<u8>>,
    operating_system: u8,
    mtime: u32,
}
impl GzHeader {
    pub fn filename(&self) -> Option<&[u8]> {
        self.filename.as_ref().map(|s| &s[..])
    }
    pub fn extra(&self) -> Option<&[u8]> {
        self.extra.as_ref().map(|s| &s[..])
    }
    pub fn comment(&self) -> Option<&[u8]> {
        self.comment.as_ref().map(|s| &s[..])
    }
    pub fn operating_system(&self) -> u8 {
        self.operating_system
    }
    pub fn mtime(&self) -> u32 {
        self.mtime
    }
    pub fn mtime_as_datetime(&self) -> Option<std::time::SystemTime> {
        self.mtime_as_duration().map(|d| std::time::UNIX_EPOCH + d)
    }
    pub fn mtime_as_duration(&self) -> Option<time::Duration> {
        if self.mtime == 0 {
            None
        } else {
            let duration = time::Duration::new(u64::from(self.mtime), 0);
            Some(duration)
        }
    }
}
#[derive(Debug, Default)]
pub enum GzHeaderState {
    Start(u8, [u8; 10]),
    Xlen(Option<Box<Crc>>, u8, [u8; 2]),
    Extra(Option<Box<Crc>>, u16),
    Filename(Option<Box<Crc>>),
    Comment(Option<Box<Crc>>),
    Crc(Option<Box<Crc>>, u8, [u8; 2]),
    #[default]
    Complete,
}
#[derive(Debug, Default)]
pub struct GzHeaderParser {
    state: GzHeaderState,
    flags: u8,
    header: GzHeader,
}
impl GzHeaderParser {
    fn new() -> Self {
        GzHeaderParser {
            state: GzHeaderState::Start(0, [0; 10]),
            flags: 0,
            header: GzHeader::default(),
        }
    }
    fn parse<R: BufRead>(&mut self, r: &mut R) -> Result<()> {
        loop {
            match &mut self.state {
                GzHeaderState::Start(count, buffer) => {
                    while (*count as usize) < buffer.len() {
                        *count += read_into(r, &mut buffer[*count as usize..])? as u8;
                    }
                    if buffer[0] != 0x1f || buffer[1] != 0x8b {
                        return Err(bad_header());
                    }
                    if buffer[2] != 8 {
                        return Err(bad_header());
                    }
                    self.flags = buffer[3];
                    if self.flags & FRESERVED != 0 {
                        return Err(bad_header());
                    }
                    self.header.mtime = (buffer[4] as u32)
                        | ((buffer[5] as u32) << 8)
                        | ((buffer[6] as u32) << 16)
                        | ((buffer[7] as u32) << 24);
                    let _xfl = buffer[8];
                    self.header.operating_system = buffer[9];
                    let crc = if self.flags & FHCRC != 0 {
                        let mut crc = Box::new(Crc::new());
                        crc.update(buffer);
                        Some(crc)
                    } else {
                        None
                    };
                    self.state = GzHeaderState::Xlen(crc, 0, [0; 2]);
                }
                GzHeaderState::Xlen(crc, count, buffer) => {
                    if self.flags & FEXTRA != 0 {
                        while (*count as usize) < buffer.len() {
                            *count += read_into(r, &mut buffer[*count as usize..])? as u8;
                        }
                        if let Some(crc) = crc {
                            crc.update(buffer);
                        }
                        let xlen = parse_le_u16(buffer);
                        self.header.extra = Some(vec![0; xlen as usize]);
                        self.state = GzHeaderState::Extra(crc.take(), 0);
                    } else {
                        self.state = GzHeaderState::Filename(crc.take());
                    }
                }
                GzHeaderState::Extra(crc, count) => {
                    debug_assert!(self.header.extra.is_some());
                    let extra = self.header.extra.as_mut().unwrap();
                    while (*count as usize) < extra.len() {
                        *count += read_into(r, &mut extra[*count as usize..])? as u16;
                    }
                    if let Some(crc) = crc {
                        crc.update(extra);
                    }
                    self.state = GzHeaderState::Filename(crc.take());
                }
                GzHeaderState::Filename(crc) => {
                    if self.flags & FNAME != 0 {
                        let filename = self.header.filename.get_or_insert_with(Vec::new);
                        read_to_nul(r, filename)?;
                        if let Some(crc) = crc {
                            crc.update(filename);
                            crc.update(b"\0");
                        }
                    }
                    self.state = GzHeaderState::Comment(crc.take());
                }
                GzHeaderState::Comment(crc) => {
                    if self.flags & FCOMMENT != 0 {
                        let comment = self.header.comment.get_or_insert_with(Vec::new);
                        read_to_nul(r, comment)?;
                        if let Some(crc) = crc {
                            crc.update(comment);
                            crc.update(b"\0");
                        }
                    }
                    self.state = GzHeaderState::Crc(crc.take(), 0, [0; 2]);
                }
                GzHeaderState::Crc(crc, count, buffer) => {
                    if let Some(crc) = crc {
                        debug_assert!(self.flags & FHCRC != 0);
                        while (*count as usize) < buffer.len() {
                            *count += read_into(r, &mut buffer[*count as usize..])? as u8;
                        }
                        let stored_crc = parse_le_u16(buffer);
                        let calced_crc = crc.sum() as u16;
                        if stored_crc != calced_crc {
                            return Err(corrupt());
                        }
                    }
                    self.state = GzHeaderState::Complete;
                }
                GzHeaderState::Complete => {
                    return Ok(());
                }
            }
        }
    }
}
impl From<GzHeaderParser> for GzHeader {
    fn from(parser: GzHeaderParser) -> Self {
        debug_assert!(matches!(parser.state, GzHeaderState::Complete));
        parser.header
    }
}
fn read_into<R: Read>(r: &mut R, buffer: &mut [u8]) -> Result<usize> {
    debug_assert!(!buffer.is_empty());
    match r.read(buffer) {
        Ok(0) => Err(ErrorKind::UnexpectedEof.into()),
        Ok(n) => Ok(n),
        Err(ref e) if e.kind() == ErrorKind::Interrupted => Ok(0),
        Err(e) => Err(e),
    }
}
fn read_to_nul<R: BufRead>(r: &mut R, buffer: &mut Vec<u8>) -> Result<()> {
    let mut bytes = r.bytes();
    loop {
        match bytes.next().transpose()? {
            Some(0) => return Ok(()),
            Some(_) if buffer.len() == MAX_HEADER_BUF => {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "gzip header field too long",
                ));
            }
            Some(byte) => {
                buffer.push(byte);
            }
            None => {
                return Err(ErrorKind::UnexpectedEof.into());
            }
        }
    }
}
fn parse_le_u16(buffer: &[u8; 2]) -> u16 {
    u16::from_le_bytes(*buffer)
}
fn bad_header() -> Error {
    Error::new(ErrorKind::InvalidInput, "invalid gzip header")
}
fn corrupt() -> Error {
    Error::new(
        ErrorKind::InvalidInput,
        "corrupt gzip stream does not have a matching checksum",
    )
}
