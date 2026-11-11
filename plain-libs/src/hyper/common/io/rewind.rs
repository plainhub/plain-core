use crate::hyper::rt::{Read, ReadBufCursor, Write};
use bytes::{Buf, Bytes};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::{cmp, io};
#[derive(Debug)]
pub(crate) struct Rewind<T> {
    pre: Option<Bytes>,
    inner: T,
}
impl<T> Rewind<T> {
    pub(crate) fn new_buffered(io: T, buf: Bytes) -> Self {
        Rewind {
            pre: Some(buf),
            inner: io,
        }
    }
    pub(crate) fn into_inner(self) -> (T, Bytes) {
        (self.inner, self.pre.unwrap_or_default())
    }
}
impl<T> Read for Rewind<T>
where
    T: Read + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        mut buf: ReadBufCursor<'_>,
    ) -> Poll<io::Result<()>> {
        if let Some(mut prefix) = self.pre.take() {
            if !prefix.is_empty() {
                let copy_len = cmp::min(prefix.len(), buf.remaining());
                buf.put_slice(&prefix[..copy_len]);
                prefix.advance(copy_len);
                if !prefix.is_empty() {
                    self.pre = Some(prefix);
                }
                return Poll::Ready(Ok(()));
            }
        }
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}
impl<T> Write for Rewind<T>
where
    T: Write + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }
    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write_vectored(cx, bufs)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }
}
