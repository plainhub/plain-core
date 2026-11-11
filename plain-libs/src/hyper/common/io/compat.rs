use std::pin::Pin;
use std::task::{Context, Poll};
#[derive(Debug)]
pub(crate) struct Compat<T>(pub(crate) T);
impl<T> Compat<T> {
    pub(crate) fn new(io: T) -> Self {
        Compat(io)
    }
    fn p(self: Pin<&mut Self>) -> Pin<&mut T> {
        unsafe { self.map_unchecked_mut(|me| &mut me.0) }
    }
}
impl<T> tokio::io::AsyncRead for Compat<T>
where
    T: crate::hyper::rt::Read,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        tbuf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        let init = tbuf.initialized().len();
        let filled = tbuf.filled().len();
        let (new_init, new_filled) = unsafe {
            let mut buf = crate::hyper::rt::ReadBuf::uninit(tbuf.inner_mut());
            buf.set_init(init);
            buf.set_filled(filled);
            match crate::hyper::rt::Read::poll_read(self.p(), cx, buf.unfilled()) {
                Poll::Ready(Ok(())) => (buf.init_len(), buf.len()),
                other => return other,
            }
        };
        let n_init = new_init - init;
        unsafe {
            tbuf.assume_init(n_init);
            tbuf.set_filled(new_filled);
        }
        Poll::Ready(Ok(()))
    }
}
impl<T> tokio::io::AsyncWrite for Compat<T>
where
    T: crate::hyper::rt::Write,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        crate::hyper::rt::Write::poll_write(self.p(), cx, buf)
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        crate::hyper::rt::Write::poll_flush(self.p(), cx)
    }
    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        crate::hyper::rt::Write::poll_shutdown(self.p(), cx)
    }
    fn is_write_vectored(&self) -> bool {
        crate::hyper::rt::Write::is_write_vectored(&self.0)
    }
    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<Result<usize, std::io::Error>> {
        crate::hyper::rt::Write::poll_write_vectored(self.p(), cx, bufs)
    }
}
