use crate::hyper::common::io::Rewind;
use crate::hyper::common::lock::LockResultExt;
use crate::hyper::rt::{Read, ReadBufCursor, Write};
use bytes::Bytes;
use std::any::TypeId;
use std::error::Error as StdError;
use std::fmt;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tokio::sync::oneshot;
pub struct Upgraded {
    io: Rewind<Box<dyn Io + Send>>,
}
#[derive(Clone)]
pub struct OnUpgrade {
    rx: Option<Arc<Mutex<oneshot::Receiver<crate::hyper::Result<Upgraded>>>>>,
}
#[derive(Debug)]
#[non_exhaustive]
pub struct Parts<T> {
    pub io: T,
    pub read_buf: Bytes,
}
pub fn on<T: sealed::CanUpgrade>(msg: T) -> OnUpgrade {
    msg.on_upgrade()
}
pub(super) struct Pending {
    tx: oneshot::Sender<crate::hyper::Result<Upgraded>>,
}
pub(super) fn pending() -> (Pending, OnUpgrade) {
    let (tx, rx) = oneshot::channel();
    (
        Pending { tx },
        OnUpgrade {
            rx: Some(Arc::new(Mutex::new(rx))),
        },
    )
}
impl Upgraded {
    pub(super) fn new<T>(io: T, read_buf: Bytes) -> Self
    where
        T: Read + Write + Unpin + Send + 'static,
    {
        Upgraded {
            io: Rewind::new_buffered(Box::new(io), read_buf),
        }
    }
    pub fn downcast<T: Read + Write + Unpin + 'static>(self) -> Result<Parts<T>, Self> {
        let (io, buf) = self.io.into_inner();
        match io.__hyper_downcast() {
            Ok(t) => Ok(Parts {
                io: *t,
                read_buf: buf,
            }),
            Err(io) => Err(Upgraded {
                io: Rewind::new_buffered(io, buf),
            }),
        }
    }
}
impl Read for Upgraded {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: ReadBufCursor<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_read(cx, buf)
    }
}
impl Write for Upgraded {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.io).poll_write(cx, buf)
    }
    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.io).poll_write_vectored(cx, bufs)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_shutdown(cx)
    }
    fn is_write_vectored(&self) -> bool {
        self.io.is_write_vectored()
    }
}
impl fmt::Debug for Upgraded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Upgraded").finish()
    }
}
impl OnUpgrade {
    pub(super) fn none() -> Self {
        OnUpgrade { rx: None }
    }
    pub(super) fn is_none(&self) -> bool {
        self.rx.is_none()
    }
}
impl Future for OnUpgrade {
    type Output = Result<Upgraded, crate::hyper::Error>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match &self.rx {
            Some(rx) => {
                Pin::new(&mut *rx.lock().panic_if_poisoned())
                    .poll(cx)
                    .map(|res| match res {
                        Ok(Ok(upgraded)) => Ok(upgraded),
                        Ok(Err(err)) => Err(err),
                        Err(_oneshot_canceled) => {
                            Err(crate::hyper::Error::new_canceled().with(UpgradeExpected))
                        }
                    })
            }
            None => Poll::Ready(Err(crate::hyper::Error::new_user_no_upgrade())),
        }
    }
}
impl fmt::Debug for OnUpgrade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OnUpgrade").finish()
    }
}
impl Pending {
    pub(super) fn fulfill(self, upgraded: Upgraded) {
        trace!("pending upgrade fulfill");
        let _ = self.tx.send(Ok(upgraded));
    }
    pub(super) fn manual(self) {
        trace!("pending upgrade handled manually");
        let _ = self
            .tx
            .send(Err(crate::hyper::Error::new_user_manual_upgrade()));
    }
}
#[derive(Debug)]
struct UpgradeExpected;
impl fmt::Display for UpgradeExpected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("upgrade expected but not completed")
    }
}
impl StdError for UpgradeExpected {}
pub(super) trait Io: Read + Write + Unpin + 'static {
    fn __hyper_type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}
impl<T: Read + Write + Unpin + 'static> Io for T {}
impl dyn Io + Send {
    fn __hyper_is<T: Io>(&self) -> bool {
        let t = TypeId::of::<T>();
        self.__hyper_type_id() == t
    }
    fn __hyper_downcast<T: Io>(self: Box<Self>) -> Result<Box<T>, Box<Self>> {
        if self.__hyper_is::<T>() {
            unsafe {
                let raw: *mut dyn Io = Box::into_raw(self);
                Ok(Box::from_raw(raw.cast()))
            }
        } else {
            Err(self)
        }
    }
}
mod sealed {
    use super::OnUpgrade;
    pub trait CanUpgrade {
        fn on_upgrade(self) -> OnUpgrade;
    }
    impl<B> CanUpgrade for crate::http::Request<B> {
        fn on_upgrade(mut self) -> OnUpgrade {
            self.extensions_mut()
                .remove::<OnUpgrade>()
                .unwrap_or_else(OnUpgrade::none)
        }
    }
    impl<B> CanUpgrade for &'_ mut crate::http::Request<B> {
        fn on_upgrade(self) -> OnUpgrade {
            self.extensions_mut()
                .remove::<OnUpgrade>()
                .unwrap_or_else(OnUpgrade::none)
        }
    }
    impl<B> CanUpgrade for crate::http::Response<B> {
        fn on_upgrade(mut self) -> OnUpgrade {
            self.extensions_mut()
                .remove::<OnUpgrade>()
                .unwrap_or_else(OnUpgrade::none)
        }
    }
    impl<B> CanUpgrade for &'_ mut crate::http::Response<B> {
        fn on_upgrade(self) -> OnUpgrade {
            self.extensions_mut()
                .remove::<OnUpgrade>()
                .unwrap_or_else(OnUpgrade::none)
        }
    }
}
