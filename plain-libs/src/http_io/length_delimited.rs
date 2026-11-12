use bytes::BytesMut;
use std::{fmt, io};

#[derive(Debug)]
pub(crate) struct LengthDelimitedCodec {
    max_frame_len: usize,
    remaining: Option<usize>,
}
#[derive(Debug)]
pub(crate) struct LengthDelimitedCodecError;
impl fmt::Display for LengthDelimitedCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("frame size too big")
    }
}
impl std::error::Error for LengthDelimitedCodecError {}
impl LengthDelimitedCodec {
    pub(crate) fn new() -> Self {
        Self {
            max_frame_len: 8 * 1024 * 1024,
            remaining: None,
        }
    }
    pub(crate) fn max_frame_length(&self) -> usize {
        self.max_frame_len
    }
    pub(crate) fn set_max_frame_length(&mut self, value: usize) {
        self.max_frame_len = value;
    }
    pub(crate) fn decode(&mut self, buf: &mut BytesMut) -> io::Result<Option<BytesMut>> {
        let len = match self.remaining {
            Some(len) => len,
            None => {
                if buf.len() < 3 {
                    return Ok(None);
                }
                let payload =
                    ((buf[0] as usize) << 16) | ((buf[1] as usize) << 8) | buf[2] as usize;
                if payload > self.max_frame_len {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        LengthDelimitedCodecError,
                    ));
                }
                let len = payload + 9;
                buf.reserve(len.saturating_sub(buf.len()));
                self.remaining = Some(len);
                len
            }
        };
        if buf.len() < len {
            return Ok(None);
        }
        let frame = buf.split_to(len);
        self.remaining = None;
        buf.reserve(3usize.saturating_sub(buf.len()));
        Ok(Some(frame))
    }
    pub(crate) fn decode_eof(&mut self, buf: &mut BytesMut) -> io::Result<Option<BytesMut>> {
        match self.decode(buf)? {
            Some(frame) => Ok(Some(frame)),
            None if buf.is_empty() => Ok(None),
            None => Err(io::Error::new(
                io::ErrorKind::Other,
                "bytes remaining on stream",
            )),
        }
    }
}
