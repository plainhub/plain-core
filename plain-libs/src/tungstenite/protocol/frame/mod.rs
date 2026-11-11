pub mod coding;
mod frame;
mod mask;
mod utf8;
pub use self::{
    frame::{CloseFrame, Frame, FrameHeader},
    utf8::Utf8Bytes,
};
use crate::tungstenite::{
    Message,
    error::{CapacityError, Error, ProtocolError, Result},
    protocol::frame::mask::apply_mask,
};
use bytes::BytesMut;
use log::*;
use std::io::{self, Cursor, Error as IoError, ErrorKind as IoErrorKind, Read, Write};
const READ_BUF_LEN: usize = 128 * 1024;
#[derive(Debug)]
pub struct FrameSocket<Stream> {
    stream: Stream,
    codec: FrameCodec,
}
impl<Stream> FrameSocket<Stream> {
    pub fn new(stream: Stream) -> Self {
        FrameSocket {
            stream,
            codec: FrameCodec::new(READ_BUF_LEN),
        }
    }
    pub fn from_partially_read(stream: Stream, part: Vec<u8>) -> Self {
        FrameSocket {
            stream,
            codec: FrameCodec::from_partially_read(part, READ_BUF_LEN),
        }
    }
    pub fn into_inner(self) -> (Stream, BytesMut) {
        (self.stream, self.codec.in_buffer)
    }
    pub fn get_ref(&self) -> &Stream {
        &self.stream
    }
    pub fn get_mut(&mut self) -> &mut Stream {
        &mut self.stream
    }
}
impl<Stream> FrameSocket<Stream>
where
    Stream: Read,
{
    pub fn read(&mut self, max_size: Option<usize>) -> Result<Option<Frame>> {
        self.codec
            .read_frame(&mut self.stream, max_size, false, true)
    }
}
impl<Stream> FrameSocket<Stream>
where
    Stream: Write,
{
    pub fn send(&mut self, frame: Frame) -> Result<()> {
        self.write(frame)?;
        self.flush()
    }
    pub fn write(&mut self, frame: Frame) -> Result<()> {
        self.codec.buffer_frame(&mut self.stream, frame)
    }
    pub fn flush(&mut self) -> Result<()> {
        self.codec.write_out_buffer(&mut self.stream)?;
        Ok(self.stream.flush()?)
    }
}
#[derive(Debug)]
pub(super) struct FrameCodec {
    in_buffer: BytesMut,
    in_buf_max_read: usize,
    out_buffer: Vec<u8>,
    max_out_buffer_len: usize,
    out_buffer_write_len: usize,
    header: Option<(FrameHeader, u64)>,
}
impl FrameCodec {
    pub(super) fn new(in_buf_len: usize) -> Self {
        Self {
            in_buffer: BytesMut::with_capacity(in_buf_len),
            in_buf_max_read: in_buf_len.max(FrameHeader::MAX_SIZE),
            out_buffer: <_>::default(),
            max_out_buffer_len: usize::MAX,
            out_buffer_write_len: 0,
            header: None,
        }
    }
    pub(super) fn from_partially_read(part: Vec<u8>, min_in_buf_len: usize) -> Self {
        let mut in_buffer = BytesMut::from_iter(part);
        in_buffer.reserve(min_in_buf_len.saturating_sub(in_buffer.len()));
        Self {
            in_buffer,
            in_buf_max_read: min_in_buf_len.max(FrameHeader::MAX_SIZE),
            out_buffer: <_>::default(),
            max_out_buffer_len: usize::MAX,
            out_buffer_write_len: 0,
            header: None,
        }
    }
    pub(super) fn set_max_out_buffer_len(&mut self, max: usize) {
        self.max_out_buffer_len = max;
    }
    pub(super) fn set_out_buffer_write_len(&mut self, len: usize) {
        self.out_buffer_write_len = len;
    }
    pub(super) fn read_frame(
        &mut self,
        stream: &mut impl Read,
        max_size: Option<usize>,
        unmask: bool,
        accept_unmasked: bool,
    ) -> Result<Option<Frame>> {
        let max_size = max_size.unwrap_or_else(usize::max_value);
        let mut payload = loop {
            if self.header.is_none() {
                let mut cursor = Cursor::new(&mut self.in_buffer);
                self.header = FrameHeader::parse(&mut cursor)?;
                let advanced = cursor.position();
                bytes::Buf::advance(&mut self.in_buffer, advanced as _);
                if let Some((_, len)) = &self.header {
                    if *len > max_size as u64 {
                        return Err(Error::Capacity(CapacityError::MessageTooLong {
                            size: *len as usize,
                            max_size,
                        }));
                    }
                    let len = *len as usize;
                    self.in_buffer.reserve(len);
                } else {
                    self.in_buffer.reserve(FrameHeader::MAX_SIZE);
                }
            }
            if let Some((_, len)) = &self.header {
                let len = *len as usize;
                if len <= self.in_buffer.len() {
                    break self.in_buffer.split_to(len);
                }
            }
            if self.read_in(stream)? == 0 {
                trace!("no frame received");
                return Ok(None);
            }
        };
        let (mut header, length) = self.header.take().expect("Bug: no frame header");
        debug_assert_eq!(payload.len() as u64, length);
        if unmask {
            if let Some(mask) = header.mask.take() {
                apply_mask(&mut payload, mask);
            } else if !accept_unmasked {
                return Err(Error::Protocol(ProtocolError::UnmaskedFrameFromClient));
            }
        }
        let frame = Frame::from_payload(header, payload.freeze());
        trace!("received frame {frame}");
        Ok(Some(frame))
    }
    fn read_in(&mut self, stream: &mut impl Read) -> io::Result<usize> {
        let len = self.in_buffer.len();
        debug_assert!(self.in_buffer.capacity() > len);
        self.in_buffer
            .resize(self.in_buffer.capacity().min(len + self.in_buf_max_read), 0);
        let size = stream.read(&mut self.in_buffer[len..]);
        self.in_buffer
            .truncate(len + size.as_ref().copied().unwrap_or(0));
        size
    }
    pub(super) fn buffer_frame<Stream>(&mut self, stream: &mut Stream, frame: Frame) -> Result<()>
    where
        Stream: Write,
    {
        if frame.len() + self.out_buffer.len() > self.max_out_buffer_len {
            return Err(Error::WriteBufferFull(Message::Frame(frame).into()));
        }
        trace!("writing frame {frame}");
        self.out_buffer.reserve(frame.len());
        frame
            .format_into_buf(&mut self.out_buffer)
            .expect("Bug: can't write to vector");
        if self.out_buffer.len() > self.out_buffer_write_len {
            self.write_out_buffer(stream)
        } else {
            Ok(())
        }
    }
    pub(super) fn write_out_buffer<Stream>(&mut self, stream: &mut Stream) -> Result<()>
    where
        Stream: Write,
    {
        while !self.out_buffer.is_empty() {
            let len = stream.write(&self.out_buffer)?;
            if len == 0 {
                return Err(IoError::new(
                    IoErrorKind::ConnectionReset,
                    "Connection reset while sending",
                )
                .into());
            }
            self.out_buffer.drain(0..len);
        }
        Ok(())
    }
}
