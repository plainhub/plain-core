#[cfg(feature = "http-runtime")]
use bytes::Buf;
use bytes::BufMut;
use std::{
    io,
    pin::Pin,
    task::{Context, Poll, ready},
};
use tokio::io::{AsyncRead, ReadBuf};

pub(crate) fn poll_read_buf<T: AsyncRead + ?Sized, B: BufMut>(
    io: Pin<&mut T>,
    cx: &mut Context<'_>,
    buf: &mut B,
) -> Poll<io::Result<usize>> {
    if !buf.has_remaining_mut() {
        return Poll::Ready(Ok(0));
    }
    let n = {
        let dst = buf.chunk_mut();
        // UninitSlice is transparent over MaybeUninit; ReadBuf exposes only initialized bytes.
        let dst = unsafe { dst.as_uninit_slice_mut() };
        let mut read = ReadBuf::uninit(dst);
        let ptr = read.filled().as_ptr();
        ready!(io.poll_read(cx, &mut read)?);
        assert_eq!(ptr, read.filled().as_ptr());
        read.filled().len()
    };
    // ReadBuf::filled establishes that exactly these bytes are initialized.
    unsafe {
        buf.advance_mut(n);
    }
    Poll::Ready(Ok(n))
}

#[cfg(feature = "http-runtime")]
pub(crate) fn poll_write_buf<T: tokio::io::AsyncWrite + ?Sized, B: Buf>(
    io: Pin<&mut T>,
    cx: &mut Context<'_>,
    buf: &mut B,
) -> Poll<io::Result<usize>> {
    const MAX_BUFS: usize = 64;
    if !buf.has_remaining() {
        return Poll::Ready(Ok(0));
    }
    let n = if io.is_write_vectored() {
        let mut slices = [io::IoSlice::new(&[]); MAX_BUFS];
        let count = buf.chunks_vectored(&mut slices);
        ready!(io.poll_write_vectored(cx, &slices[..count]))?
    } else {
        ready!(io.poll_write(cx, buf.chunk()))?
    };
    buf.advance(n);
    Poll::Ready(Ok(n))
}

#[cfg(all(test, feature = "http-runtime"))]
#[path = "../../tests/plain-libs/unit/io_buf.rs"]
mod tests;
