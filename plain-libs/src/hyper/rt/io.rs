use std::fmt;
use std::mem::MaybeUninit;
use std::ops::DerefMut;
use std::pin::Pin;
use std::task::{Context, Poll};
pub trait Read {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: ReadBufCursor<'_>,
    ) -> Poll<Result<(), std::io::Error>>;
}
pub trait Write {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>>;
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>>;
    fn poll_shutdown(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>>;
    fn is_write_vectored(&self) -> bool {
        false
    }
    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<Result<usize, std::io::Error>> {
        let buf = bufs
            .iter()
            .find(|b| !b.is_empty())
            .map_or(&[][..], |b| &**b);
        self.poll_write(cx, buf)
    }
}
pub struct ReadBuf<'a> {
    raw: &'a mut [MaybeUninit<u8>],
    filled: usize,
    init: usize,
}
#[derive(Debug)]
pub struct ReadBufCursor<'a> {
    buf: &'a mut ReadBuf<'a>,
}
impl<'data> ReadBuf<'data> {
    #[inline]
    pub fn new(raw: &'data mut [u8]) -> Self {
        let len = raw.len();
        Self {
            raw: unsafe { &mut *(raw as *mut [u8] as *mut [MaybeUninit<u8>]) },
            filled: 0,
            init: len,
        }
    }
    #[inline]
    pub fn uninit(raw: &'data mut [MaybeUninit<u8>]) -> Self {
        Self {
            raw,
            filled: 0,
            init: 0,
        }
    }
    #[inline]
    pub fn filled(&self) -> &[u8] {
        unsafe { &*(std::ptr::addr_of!(self.raw[0..self.filled]) as *const [u8]) }
    }
    #[inline]
    pub fn unfilled<'cursor>(&'cursor mut self) -> ReadBufCursor<'cursor> {
        ReadBufCursor {
            buf: unsafe {
                std::mem::transmute::<&'cursor mut ReadBuf<'data>, &'cursor mut ReadBuf<'cursor>>(
                    self,
                )
            },
        }
    }
    #[inline]
    pub(crate) unsafe fn set_init(&mut self, n: usize) {
        self.init = self.init.max(n);
    }
    #[inline]
    pub(crate) unsafe fn set_filled(&mut self, n: usize) {
        self.filled = self.filled.max(n);
    }
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.filled
    }
    #[inline]
    pub(crate) fn init_len(&self) -> usize {
        self.init
    }
    #[inline]
    fn remaining(&self) -> usize {
        self.capacity() - self.filled
    }
    #[inline]
    fn capacity(&self) -> usize {
        self.raw.len()
    }
}
impl fmt::Debug for ReadBuf<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReadBuf")
            .field("filled", &self.filled)
            .field("init", &self.init)
            .field("capacity", &self.capacity())
            .finish()
    }
}
impl ReadBufCursor<'_> {
    #[inline]
    pub unsafe fn as_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        &mut self.buf.raw[self.buf.filled..]
    }
    #[inline]
    pub unsafe fn advance(&mut self, n: usize) {
        self.buf.filled = self.buf.filled.checked_add(n).expect("overflow");
        self.buf.init = self.buf.filled.max(self.buf.init);
    }
    #[inline]
    pub fn remaining(&self) -> usize {
        self.buf.remaining()
    }
    #[inline]
    pub fn put_slice(&mut self, src: &[u8]) {
        assert!(
            self.buf.remaining() >= src.len(),
            "src.len() must fit in remaining()"
        );
        let amt = src.len();
        let end = self.buf.filled + amt;
        unsafe {
            self.buf.raw[self.buf.filled..end]
                .as_mut_ptr()
                .cast::<u8>()
                .copy_from_nonoverlapping(src.as_ptr(), amt);
        }
        if self.buf.init < end {
            self.buf.init = end;
        }
        self.buf.filled = end;
    }

    #[inline]
    pub fn initialize_unfilled_to(&mut self, n: usize) -> &mut [u8] {
        assert!(self.remaining() >= n, "n overflows remaining");
        let end = self.buf.filled + n;
        if self.buf.init < end {
            unsafe {
                self.buf.raw[self.buf.init..end]
                    .as_mut_ptr()
                    .write_bytes(0, end - self.buf.init);
            }
            self.buf.init = end;
        }
        let slice = &mut self.buf.raw[self.buf.filled..end];
        unsafe { &mut *(slice as *mut [MaybeUninit<u8>] as *mut [u8]) }
    }
}
macro_rules! deref_async_read {
    () => {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: ReadBufCursor<'_>,
        ) -> Poll<std::io::Result<()>> {
            Pin::new(&mut **self).poll_read(cx, buf)
        }
    };
}
impl<T: ?Sized + Read + Unpin> Read for Box<T> {
    deref_async_read!();
}
impl<T: ?Sized + Read + Unpin> Read for &mut T {
    deref_async_read!();
}
impl<P> Read for Pin<P>
where
    P: DerefMut,
    P::Target: Read,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: ReadBufCursor<'_>,
    ) -> Poll<std::io::Result<()>> {
        pin_as_deref_mut(self).poll_read(cx, buf)
    }
}
macro_rules! deref_async_write {
    () => {
        fn poll_write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            Pin::new(&mut **self).poll_write(cx, buf)
        }
        fn poll_write_vectored(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            bufs: &[std::io::IoSlice<'_>],
        ) -> Poll<std::io::Result<usize>> {
            Pin::new(&mut **self).poll_write_vectored(cx, bufs)
        }
        fn is_write_vectored(&self) -> bool {
            (**self).is_write_vectored()
        }
        fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Pin::new(&mut **self).poll_flush(cx)
        }
        fn poll_shutdown(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<std::io::Result<()>> {
            Pin::new(&mut **self).poll_shutdown(cx)
        }
    };
}
impl<T: ?Sized + Write + Unpin> Write for Box<T> {
    deref_async_write!();
}
impl<T: ?Sized + Write + Unpin> Write for &mut T {
    deref_async_write!();
}
impl<P> Write for Pin<P>
where
    P: DerefMut,
    P::Target: Write,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        pin_as_deref_mut(self).poll_write(cx, buf)
    }
    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[std::io::IoSlice<'_>],
    ) -> Poll<std::io::Result<usize>> {
        pin_as_deref_mut(self).poll_write_vectored(cx, bufs)
    }
    fn is_write_vectored(&self) -> bool {
        (**self).is_write_vectored()
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        pin_as_deref_mut(self).poll_flush(cx)
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        pin_as_deref_mut(self).poll_shutdown(cx)
    }
}
fn pin_as_deref_mut<P: DerefMut>(pin: Pin<&mut Pin<P>>) -> Pin<&mut P::Target> {
    unsafe { pin.get_unchecked_mut() }.as_mut()
}
