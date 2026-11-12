use crate::h2::codec::UserError;
use crate::h2::codec::UserError::*;
use crate::h2::frame::{self, Frame, FrameSize};
use crate::h2::hpack;
use crate::http_io::poll_write_buf;
use bytes::{Buf, BufMut, BytesMut};
use std::io::{self, Cursor};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
macro_rules! limited_write_buf {
    ($self : expr) => {{
        let limit = $self.max_frame_size() + frame::HEADER_LEN;
        $self.buf.get_mut().limit(limit)
    }};
}
#[derive(Debug)]
pub struct FramedWrite<T, B> {
    inner: T,
    final_flush_done: bool,
    encoder: Encoder<B>,
}
#[derive(Debug)]
struct Encoder<B> {
    hpack: hpack::Encoder,
    buf: Cursor<BytesMut>,
    next: Option<Next<B>>,
    last_data_frame: Option<frame::Data<B>>,
    max_frame_size: FrameSize,
    chain_threshold: usize,
    min_buffer_capacity: usize,
}
#[derive(Debug)]
enum Next<B> {
    Data(frame::Data<B>),
    Continuation(frame::Continuation),
}
const DEFAULT_BUFFER_CAPACITY: usize = 16 * 1_024;
const CHAIN_THRESHOLD: usize = 256;
const CHAIN_THRESHOLD_WITHOUT_VECTORED_IO: usize = 1024;
impl<T, B> FramedWrite<T, B>
where
    T: AsyncWrite + Unpin,
    B: Buf,
{
    pub fn new(inner: T) -> FramedWrite<T, B> {
        let chain_threshold = if inner.is_write_vectored() {
            CHAIN_THRESHOLD
        } else {
            CHAIN_THRESHOLD_WITHOUT_VECTORED_IO
        };
        FramedWrite {
            inner,
            final_flush_done: false,
            encoder: Encoder {
                hpack: hpack::Encoder::default(),
                buf: Cursor::new(BytesMut::with_capacity(DEFAULT_BUFFER_CAPACITY)),
                next: None,
                last_data_frame: None,
                max_frame_size: frame::DEFAULT_MAX_FRAME_SIZE,
                chain_threshold,
                min_buffer_capacity: chain_threshold + frame::HEADER_LEN,
            },
        }
    }
    pub fn poll_ready(&mut self, cx: &mut Context) -> Poll<io::Result<()>> {
        if !self.encoder.has_capacity() {
            ready!(self.flush(cx))?;
            if !self.encoder.has_capacity() {
                return Poll::Pending;
            }
        }
        Poll::Ready(Ok(()))
    }
    pub(crate) fn has_capacity(&self) -> bool {
        self.encoder.has_capacity()
    }
    pub fn buffer(&mut self, item: Frame<B>) -> Result<(), UserError> {
        self.encoder.buffer(item)
    }
    pub fn flush(&mut self, cx: &mut Context) -> Poll<io::Result<()>> {
        let span = tracing::trace_span!("FramedWrite::flush");
        let _e = span.enter();
        loop {
            while !self.encoder.is_empty() {
                let n = match self.encoder.next {
                    Some(Next::Data(ref mut frame)) => {
                        tracing::trace!(queued_data_frame = true);
                        let mut buf = (&mut self.encoder.buf).chain(frame.payload_mut());
                        ready!(poll_write_buf(Pin::new(&mut self.inner), cx, &mut buf))?
                    }
                    _ => {
                        tracing::trace!(queued_data_frame = false);
                        ready!(poll_write_buf(
                            Pin::new(&mut self.inner),
                            cx,
                            &mut self.encoder.buf
                        ))?
                    }
                };
                if n == 0 {
                    tracing::trace!("write returned zero, but non-zero bytes remaining");
                    return Poll::Ready(Err(io::ErrorKind::WriteZero.into()));
                }
            }
            match self.encoder.unset_frame() {
                ControlFlow::Continue => (),
                ControlFlow::Break => break,
            }
        }
        tracing::trace!("flushing buffer");
        ready!(Pin::new(&mut self.inner).poll_flush(cx))?;
        Poll::Ready(Ok(()))
    }
    pub fn shutdown(&mut self, cx: &mut Context) -> Poll<io::Result<()>> {
        if !self.final_flush_done {
            ready!(self.flush(cx))?;
            self.final_flush_done = true;
        }
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}
#[must_use]
enum ControlFlow {
    Continue,
    Break,
}
impl<B> Encoder<B>
where
    B: Buf,
{
    fn unset_frame(&mut self) -> ControlFlow {
        self.buf.set_position(0);
        self.buf.get_mut().clear();
        match self.next.take() {
            Some(Next::Data(frame)) => {
                self.last_data_frame = Some(frame);
                debug_assert!(self.is_empty());
                ControlFlow::Break
            }
            Some(Next::Continuation(frame)) => {
                let mut buf = limited_write_buf!(self);
                if let Some(continuation) = frame.encode(&mut buf) {
                    self.next = Some(Next::Continuation(continuation));
                }
                ControlFlow::Continue
            }
            None => ControlFlow::Break,
        }
    }
    fn buffer(&mut self, item: Frame<B>) -> Result<(), UserError> {
        assert!(self.has_capacity());
        let span = tracing::trace_span ! ("FramedWrite::buffer" , frame = ? item);
        let _e = span.enter();
        tracing::debug ! (frame = ? item , "send");
        match item {
            Frame::Data(mut v) => {
                let len = v.payload().remaining();
                if len > self.max_frame_size() {
                    return Err(PayloadTooBig);
                }
                if len >= self.chain_threshold {
                    let head = v.head();
                    head.encode(len, self.buf.get_mut());
                    if self.buf.get_ref().remaining() < self.chain_threshold {
                        let extra_bytes = self.chain_threshold - self.buf.remaining();
                        self.buf.get_mut().put(v.payload_mut().take(extra_bytes));
                    }
                    self.next = Some(Next::Data(v));
                } else {
                    v.encode_chunk(self.buf.get_mut());
                    assert_eq!(v.payload().remaining(), 0, "chunk not fully encoded");
                    self.last_data_frame = Some(v);
                }
            }
            Frame::Headers(v) => {
                let mut buf = limited_write_buf!(self);
                if let Some(continuation) = v.encode(&mut self.hpack, &mut buf) {
                    self.next = Some(Next::Continuation(continuation));
                }
            }
            Frame::PushPromise(v) => {
                let mut buf = limited_write_buf!(self);
                if let Some(continuation) = v.encode(&mut self.hpack, &mut buf) {
                    self.next = Some(Next::Continuation(continuation));
                }
            }
            Frame::Settings(v) => {
                v.encode(self.buf.get_mut());
                tracing::trace!(rem = self.buf.remaining(), "encoded settings");
            }
            Frame::GoAway(v) => {
                v.encode(self.buf.get_mut());
                tracing::trace!(rem = self.buf.remaining(), "encoded go_away");
            }
            Frame::Ping(v) => {
                v.encode(self.buf.get_mut());
                tracing::trace!(rem = self.buf.remaining(), "encoded ping");
            }
            Frame::WindowUpdate(v) => {
                v.encode(self.buf.get_mut());
                tracing::trace!(rem = self.buf.remaining(), "encoded window_update");
            }
            Frame::Priority(_) => {
                unimplemented!();
            }
            Frame::Reset(v) => {
                v.encode(self.buf.get_mut());
                tracing::trace!(rem = self.buf.remaining(), "encoded reset");
            }
        }
        Ok(())
    }
    fn has_capacity(&self) -> bool {
        self.next.is_none()
            && (self.buf.get_ref().capacity() - self.buf.get_ref().len()
                >= self.min_buffer_capacity)
    }
    fn is_empty(&self) -> bool {
        match self.next {
            Some(Next::Data(ref frame)) => !frame.payload().has_remaining(),
            _ => !self.buf.has_remaining(),
        }
    }
}
impl<B> Encoder<B> {
    fn max_frame_size(&self) -> usize {
        self.max_frame_size as usize
    }
}
impl<T, B> FramedWrite<T, B> {
    pub fn max_frame_size(&self) -> usize {
        self.encoder.max_frame_size()
    }
    pub fn set_max_frame_size(&mut self, val: usize) {
        assert!(val <= frame::MAX_MAX_FRAME_SIZE as usize);
        self.encoder.max_frame_size = val as FrameSize;
    }
    pub fn set_header_table_size(&mut self, val: usize) {
        self.encoder.hpack.update_max_size(val);
    }
    pub fn take_last_data_frame(&mut self) -> Option<frame::Data<B>> {
        self.encoder.last_data_frame.take()
    }
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}
impl<T: AsyncRead + Unpin, B> AsyncRead for FramedWrite<T, B> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}
impl<T: Unpin, B> Unpin for FramedWrite<T, B> {}
