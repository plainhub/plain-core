use super::{
    coding::{CloseCode, Control, Data, OpCode},
    mask::{apply_mask, generate_mask},
};
use crate::tungstenite::{
    error::{Error, ProtocolError, Result},
    protocol::frame::Utf8Bytes,
};
use bytes::{Bytes, BytesMut};
use log::*;
use std::{
    default::Default,
    fmt,
    io::{Cursor, ErrorKind, Read, Write},
    mem,
    result::Result as StdResult,
    str::Utf8Error,
    string::String,
};
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CloseFrame {
    pub code: CloseCode,
    pub reason: Utf8Bytes,
}
impl fmt::Display for CloseFrame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} ({})", self.reason, self.code)
    }
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FrameHeader {
    pub is_final: bool,
    pub rsv1: bool,
    pub rsv2: bool,
    pub rsv3: bool,
    pub opcode: OpCode,
    pub mask: Option<[u8; 4]>,
}
impl Default for FrameHeader {
    fn default() -> Self {
        FrameHeader {
            is_final: true,
            rsv1: false,
            rsv2: false,
            rsv3: false,
            opcode: OpCode::Control(Control::Close),
            mask: None,
        }
    }
}
impl FrameHeader {
    pub(crate) const MAX_SIZE: usize = 14;
    pub fn parse(cursor: &mut Cursor<impl AsRef<[u8]>>) -> Result<Option<(Self, u64)>> {
        let initial = cursor.position();
        match Self::parse_internal(cursor) {
            ret @ Ok(None) => {
                cursor.set_position(initial);
                ret
            }
            ret => ret,
        }
    }
    pub fn len(&self, length: u64) -> usize {
        2 + LengthFormat::for_length(length).extra_bytes() + if self.mask.is_some() { 4 } else { 0 }
    }
    pub fn format(&self, length: u64, output: &mut impl Write) -> Result<()> {
        let code: u8 = self.opcode.into();
        let one = {
            code | if self.is_final { 0x80 } else { 0 }
                | if self.rsv1 { 0x40 } else { 0 }
                | if self.rsv2 { 0x20 } else { 0 }
                | if self.rsv3 { 0x10 } else { 0 }
        };
        let lenfmt = LengthFormat::for_length(length);
        let two = { lenfmt.length_byte() | if self.mask.is_some() { 0x80 } else { 0 } };
        output.write_all(&[one, two])?;
        match lenfmt {
            LengthFormat::U8(_) => (),
            LengthFormat::U16 => {
                output.write_all(&(length as u16).to_be_bytes())?;
            }
            LengthFormat::U64 => {
                output.write_all(&length.to_be_bytes())?;
            }
        }
        if let Some(ref mask) = self.mask {
            output.write_all(mask)?;
        }
        Ok(())
    }
    pub(crate) fn set_random_mask(&mut self) {
        self.mask = Some(generate_mask());
    }
}
impl FrameHeader {
    fn parse_internal(cursor: &mut impl Read) -> Result<Option<(Self, u64)>> {
        let (first, second) = {
            let mut head = [0u8; 2];
            if cursor.read(&mut head)? != 2 {
                return Ok(None);
            }
            trace!("Parsed headers {head:?}");
            (head[0], head[1])
        };
        trace!("First: {first:b}");
        trace!("Second: {second:b}");
        let is_final = first & 0x80 != 0;
        let rsv1 = first & 0x40 != 0;
        let rsv2 = first & 0x20 != 0;
        let rsv3 = first & 0x10 != 0;
        let opcode = OpCode::from(first & 0x0F);
        trace!("Opcode: {opcode:?}");
        let masked = second & 0x80 != 0;
        trace!("Masked: {masked:?}");
        let length = {
            let length_byte = second & 0x7F;
            let length_length = LengthFormat::for_byte(length_byte).extra_bytes();
            if length_length > 0 {
                const SIZE: usize = mem::size_of::<u64>();
                assert!(length_length <= SIZE, "length exceeded size of u64");
                let start = SIZE - length_length;
                let mut buffer = [0; SIZE];
                match cursor.read_exact(&mut buffer[start..]) {
                    Err(ref err) if err.kind() == ErrorKind::UnexpectedEof => return Ok(None),
                    Err(err) => return Err(err.into()),
                    Ok(()) => u64::from_be_bytes(buffer),
                }
            } else {
                u64::from(length_byte)
            }
        };
        let mask = if masked {
            let mut mask_bytes = [0u8; 4];
            if cursor.read(&mut mask_bytes)? != 4 {
                return Ok(None);
            } else {
                Some(mask_bytes)
            }
        } else {
            None
        };
        match opcode {
            OpCode::Control(Control::Reserved(_)) | OpCode::Data(Data::Reserved(_)) => {
                return Err(Error::Protocol(ProtocolError::InvalidOpcode(first & 0x0F)));
            }
            _ => (),
        }
        let hdr = FrameHeader {
            is_final,
            rsv1,
            rsv2,
            rsv3,
            opcode,
            mask,
        };
        Ok(Some((hdr, length)))
    }
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Frame {
    header: FrameHeader,
    payload: Bytes,
}
impl Frame {
    #[inline]
    pub fn len(&self) -> usize {
        let length = self.payload.len();
        self.header.len(length as u64) + length
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    #[inline]
    pub fn header(&self) -> &FrameHeader {
        &self.header
    }

    #[inline]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
    #[inline]
    pub(crate) fn is_masked(&self) -> bool {
        self.header.mask.is_some()
    }
    #[inline]
    pub(crate) fn set_random_mask(&mut self) {
        self.header.set_random_mask();
    }
    #[inline]
    pub fn into_text(self) -> StdResult<Utf8Bytes, Utf8Error> {
        self.payload.try_into()
    }
    #[inline]
    pub fn into_payload(self) -> Bytes {
        self.payload
    }
    #[inline]
    pub fn to_text(&self) -> Result<&str, Utf8Error> {
        std::str::from_utf8(&self.payload)
    }
    #[inline]
    pub(crate) fn into_close(self) -> Result<Option<CloseFrame>> {
        match self.payload.len() {
            0 => Ok(None),
            1 => Err(Error::Protocol(ProtocolError::InvalidCloseSequence)),
            _ => {
                let code = u16::from_be_bytes([self.payload[0], self.payload[1]]).into();
                let reason = Utf8Bytes::try_from(self.payload.slice(2..))?;
                Ok(Some(CloseFrame { code, reason }))
            }
        }
    }
    #[inline]
    pub fn message(data: impl Into<Bytes>, opcode: OpCode, is_final: bool) -> Frame {
        debug_assert!(
            matches!(opcode, OpCode::Data(_)),
            "Invalid opcode for data frame."
        );
        Frame {
            header: FrameHeader {
                is_final,
                opcode,
                ..FrameHeader::default()
            },
            payload: data.into(),
        }
    }
    #[inline]
    pub fn pong(data: impl Into<Bytes>) -> Frame {
        Frame {
            header: FrameHeader {
                opcode: OpCode::Control(Control::Pong),
                ..FrameHeader::default()
            },
            payload: data.into(),
        }
    }
    #[inline]
    pub fn ping(data: impl Into<Bytes>) -> Frame {
        Frame {
            header: FrameHeader {
                opcode: OpCode::Control(Control::Ping),
                ..FrameHeader::default()
            },
            payload: data.into(),
        }
    }
    #[inline]
    pub fn close(msg: Option<CloseFrame>) -> Frame {
        let payload = if let Some(CloseFrame { code, reason }) = msg {
            let mut p = BytesMut::with_capacity(reason.len() + 2);
            p.extend(u16::from(code).to_be_bytes());
            p.extend_from_slice(reason.as_bytes());
            p
        } else {
            <_>::default()
        };
        Frame {
            header: FrameHeader::default(),
            payload: payload.into(),
        }
    }
    pub fn from_payload(header: FrameHeader, payload: Bytes) -> Self {
        Frame { header, payload }
    }
    pub fn format(mut self, output: &mut impl Write) -> Result<()> {
        self.header.format(self.payload.len() as u64, output)?;
        if let Some(mask) = self.header.mask.take() {
            let mut data = Vec::from(mem::take(&mut self.payload));
            apply_mask(&mut data, mask);
            output.write_all(&data)?;
        } else {
            output.write_all(&self.payload)?;
        }
        Ok(())
    }
    pub(crate) fn format_into_buf(mut self, buf: &mut Vec<u8>) -> Result<()> {
        self.header.format(self.payload.len() as u64, buf)?;
        let len = buf.len();
        buf.extend_from_slice(&self.payload);
        if let Some(mask) = self.header.mask.take() {
            apply_mask(&mut buf[len..], mask);
        }
        Ok(())
    }
}
impl fmt::Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use std::fmt::Write;
        write!(
            f,
            "
<FRAME>
final: {}
reserved: {} {} {}
opcode: {}
length: {}
payload length: {}
payload: 0x{}
            ",
            self.header.is_final,
            self.header.rsv1,
            self.header.rsv2,
            self.header.rsv3,
            self.header.opcode,
            self.len(),
            self.payload.len(),
            self.payload.iter().fold(String::new(), |mut output, byte| {
                _ = write!(output, "{byte:02x}");
                output
            })
        )
    }
}
enum LengthFormat {
    U8(u8),
    U16,
    U64,
}
impl LengthFormat {
    #[inline]
    fn for_length(length: u64) -> Self {
        if length < 126 {
            LengthFormat::U8(length as u8)
        } else if length < 65536 {
            LengthFormat::U16
        } else {
            LengthFormat::U64
        }
    }
    #[inline]
    fn extra_bytes(&self) -> usize {
        match *self {
            LengthFormat::U8(_) => 0,
            LengthFormat::U16 => 2,
            LengthFormat::U64 => 8,
        }
    }
    #[inline]
    fn length_byte(&self) -> u8 {
        match *self {
            LengthFormat::U8(b) => b,
            LengthFormat::U16 => 126,
            LengthFormat::U64 => 127,
        }
    }
    #[inline]
    fn for_byte(byte: u8) -> Self {
        match byte & 0x7F {
            126 => LengthFormat::U16,
            127 => LengthFormat::U64,
            b => LengthFormat::U8(b),
        }
    }
}
