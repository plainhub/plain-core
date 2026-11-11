use crate::tungstenite::Error as WsError;
use futures_util::task;
use log::*;
use std::sync::Arc;
use std::{
    io::{Read, Write},
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
pub(crate) enum ContextWaker {
    Read,
    Write,
}
#[derive(Debug)]
pub(crate) struct AllowStd<S> {
    inner: S,
    write_waker_proxy: Arc<WakerProxy>,
    read_waker_proxy: Arc<WakerProxy>,
}
pub(crate) trait SetWaker {
    fn set_waker(&self, waker: &task::Waker);
}
impl<S> SetWaker for AllowStd<S> {
    fn set_waker(&self, waker: &task::Waker) {
        self.set_waker(ContextWaker::Read, waker);
    }
}
impl<S> AllowStd<S> {
    pub(crate) fn new(inner: S, waker: &task::Waker) -> Self {
        let res = Self {
            inner,
            write_waker_proxy: Default::default(),
            read_waker_proxy: Default::default(),
        };
        res.write_waker_proxy.read_waker.register(waker);
        res.read_waker_proxy.read_waker.register(waker);
        res
    }
    pub(crate) fn set_waker(&self, kind: ContextWaker, waker: &task::Waker) {
        match kind {
            ContextWaker::Read => {
                self.write_waker_proxy.read_waker.register(waker);
                self.read_waker_proxy.read_waker.register(waker);
            }
            ContextWaker::Write => {
                self.write_waker_proxy.write_waker.register(waker);
                self.read_waker_proxy.write_waker.register(waker);
            }
        }
    }
    pub(crate) fn into_inner(self) -> S {
        self.inner
    }
}
#[derive(Debug, Default)]
struct WakerProxy {
    read_waker: task::AtomicWaker,
    write_waker: task::AtomicWaker,
}
impl task::ArcWake for WakerProxy {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        arc_self.read_waker.wake();
        arc_self.write_waker.wake();
    }
}
impl<S> AllowStd<S>
where
    S: Unpin,
{
    fn with_context<F, R>(&mut self, kind: ContextWaker, f: F) -> Poll<std::io::Result<R>>
    where
        F: FnOnce(&mut Context<'_>, Pin<&mut S>) -> Poll<std::io::Result<R>>,
    {
        trace!("{}:{} AllowStd.with_context", file!(), line!());
        let waker = match kind {
            ContextWaker::Read => task::waker_ref(&self.read_waker_proxy),
            ContextWaker::Write => task::waker_ref(&self.write_waker_proxy),
        };
        let mut context = task::Context::from_waker(&waker);
        f(&mut context, Pin::new(&mut self.inner))
    }
    pub(crate) fn get_mut(&mut self) -> &mut S {
        &mut self.inner
    }
    pub(crate) fn get_ref(&self) -> &S {
        &self.inner
    }
}
impl<S> Read for AllowStd<S>
where
    S: AsyncRead + Unpin,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        trace!("{}:{} Read.read", file!(), line!());
        let mut buf = ReadBuf::new(buf);
        match self.with_context(ContextWaker::Read, |ctx, stream| {
            trace!(
                "{}:{} Read.with_context read -> poll_read",
                file!(),
                line!()
            );
            stream.poll_read(ctx, &mut buf)
        }) {
            Poll::Ready(Ok(_)) => Ok(buf.filled().len()),
            Poll::Ready(Err(err)) => Err(err),
            Poll::Pending => Err(std::io::Error::from(std::io::ErrorKind::WouldBlock)),
        }
    }
}
impl<S> Write for AllowStd<S>
where
    S: AsyncWrite + Unpin,
{
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        trace!("{}:{} Write.write", file!(), line!());
        match self.with_context(ContextWaker::Write, |ctx, stream| {
            trace!(
                "{}:{} Write.with_context write -> poll_write",
                file!(),
                line!()
            );
            stream.poll_write(ctx, buf)
        }) {
            Poll::Ready(r) => r,
            Poll::Pending => Err(std::io::Error::from(std::io::ErrorKind::WouldBlock)),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        trace!("{}:{} Write.flush", file!(), line!());
        match self.with_context(ContextWaker::Write, |ctx, stream| {
            trace!(
                "{}:{} Write.with_context flush -> poll_flush",
                file!(),
                line!()
            );
            stream.poll_flush(ctx)
        }) {
            Poll::Ready(r) => r,
            Poll::Pending => Err(std::io::Error::from(std::io::ErrorKind::WouldBlock)),
        }
    }
}
pub(crate) fn cvt<T>(r: Result<T, WsError>) -> Poll<Result<T, WsError>> {
    match r {
        Ok(v) => Poll::Ready(Ok(v)),
        Err(WsError::Io(ref e)) if e.kind() == std::io::ErrorKind::WouldBlock => {
            trace!("WouldBlock");
            Poll::Pending
        }
        Err(e) => Poll::Ready(Err(e)),
    }
}
