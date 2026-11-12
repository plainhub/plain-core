use super::LengthDelimitedCodec;
use bytes::BytesMut;
use futures_core::Stream;
use std::{
    io,
    pin::Pin,
    task::{Context, Poll, ready},
};
use tokio::io::AsyncRead;

#[derive(Debug)]
pub(crate) struct FramedRead<T> {
    inner: T,
    codec: LengthDelimitedCodec,
    eof: bool,
    readable: bool,
    errored: bool,
    buffer: BytesMut,
}
impl<T> FramedRead<T> {
    pub(crate) fn new(inner: T, codec: LengthDelimitedCodec) -> Self {
        Self {
            inner,
            codec,
            eof: false,
            readable: false,
            errored: false,
            buffer: BytesMut::with_capacity(8 * 1024),
        }
    }
    pub(crate) fn get_ref(&self) -> &T {
        &self.inner
    }
    pub(crate) fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }
    pub(crate) fn decoder(&self) -> &LengthDelimitedCodec {
        &self.codec
    }
    pub(crate) fn decoder_mut(&mut self) -> &mut LengthDelimitedCodec {
        &mut self.codec
    }
}
impl<T: AsyncRead + Unpin> Stream for FramedRead<T> {
    type Item = io::Result<BytesMut>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        loop {
            if this.errored {
                this.readable = false;
                this.errored = false;
                return Poll::Ready(None);
            }
            if this.readable {
                if this.eof {
                    let frame = this.codec.decode_eof(&mut this.buffer).map_err(|err| {
                        this.errored = true;
                        err
                    })?;
                    if frame.is_none() {
                        this.readable = false;
                    }
                    return Poll::Ready(frame.map(Ok));
                }
                if let Some(frame) = this.codec.decode(&mut this.buffer).map_err(|err| {
                    this.errored = true;
                    err
                })? {
                    return Poll::Ready(Some(Ok(frame)));
                }
                this.readable = false;
            }
            this.buffer.reserve(1);
            let bytes = ready!(
                crate::io_buf::poll_read_buf(Pin::new(&mut this.inner), cx, &mut this.buffer)
                    .map_err(|err| {
                        this.errored = true;
                        err
                    })?
            );
            if bytes == 0 {
                if this.eof {
                    return Poll::Ready(None);
                }
                this.eof = true;
            } else {
                this.eof = false;
            }
            this.readable = true;
        }
    }
}
