//! Adapter from `tokio::io::AsyncRead` to `futures_core::Stream<Item = Result<Bytes, io::Error>>`.
//!
//! Ported from the previous Go implementation `src/async_read_stream.rs`, which replaced its
//! single use of `tokio_util::io::ReaderStream` to avoid pulling in
//! `tokio-util` (and its dependency fan-out) for one call site.
//!
//! Behaviour matches `tokio_util::io::ReaderStream::with_capacity(r, 64 KiB)`:
//! read up to 64 KiB per `poll_next`, yield each chunk as a `Bytes`, end
//! with `None` on EOF. Like `ReaderStream`, the read goes **directly into
//! the `BytesMut` that becomes the yielded chunk** (kernel → buffer, no
//! intermediate copy); this is the streaming hot path for `/fs` file
//! serving on weak CPUs.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::{BufMut, Bytes, BytesMut};
use futures_core::Stream;
use tokio::io::AsyncRead;

/// Chunk size per `poll_next`.
const CAPACITY: usize = 64 * 1024;

/// Adapter that turns an `AsyncRead` into a `Stream<Item = Result<Bytes, io::Error>>`.
pub struct AsyncReadStream<R> {
    reader: R,
    buf: BytesMut,
}

impl<R: AsyncRead + Unpin> AsyncReadStream<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            buf: BytesMut::with_capacity(CAPACITY),
        }
    }
}

impl<R: AsyncRead + Unpin> Stream for AsyncReadStream<R> {
    type Item = Result<Bytes, io::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        use std::task::ready;
        let this = self.get_mut();
        if !this.buf.has_remaining_mut() {
            this.buf.reserve(CAPACITY);
        }
        let n = {
            let dst = this.buf.chunk_mut();
            // SAFETY: `UninitSlice` is a transparent wrapper around
            // `[MaybeUninit<u8>]` (same conversion tokio-util uses); we
            // only hand it to `ReadBuf`, which never reads the
            // uninitialized bytes, only writes into them.
            let dst = unsafe { dst.as_uninit_slice_mut() };
            let mut read_buf = tokio::io::ReadBuf::uninit(dst);
            let ptr = read_buf.filled().as_ptr();
            ready!(Pin::new(&mut this.reader).poll_read(cx, &mut read_buf)?);
            // The pointer must not have moved from under us.
            assert_eq!(ptr, read_buf.filled().as_ptr());
            read_buf.filled().len()
        };
        if n == 0 {
            return Poll::Ready(None); // EOF
        }
        // SAFETY: `poll_read` reported exactly `n` bytes of the spare
        // capacity as initialized, so advancing by that amount is sound.
        unsafe { this.buf.advance_mut(n) };
        Poll::Ready(Some(Ok(this.buf.split().freeze())))
    }
}

#[cfg(test)]
#[path = "../../tests/unit/utils/async_read_stream.rs"]
mod tests;
