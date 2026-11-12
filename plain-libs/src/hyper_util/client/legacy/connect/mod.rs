pub use self::http::{HttpConnector, HttpInfo};
use crate::http::Extensions;
use std::{
    fmt::{self, Formatter},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
pub(crate) mod capture;
pub mod dns;
mod http;
pub mod proxy;
pub use self::sealed::Connect;
pub use capture::{CaptureConnection, capture_connection};
pub trait Connection {
    fn connected(&self) -> Connected;
}
#[derive(Debug)]
pub struct Connected {
    pub(super) alpn: Alpn,
    pub(super) is_proxied: bool,
    pub(super) extra: Option<Extra>,
    pub(super) poisoned: PoisonPill,
}
#[derive(Clone)]
pub(crate) struct PoisonPill {
    poisoned: Arc<AtomicBool>,
}
impl fmt::Debug for PoisonPill {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PoisonPill@{:p} {{ poisoned: {} }}",
            self.poisoned,
            self.poisoned.load(Ordering::Relaxed)
        )
    }
}
impl PoisonPill {
    pub(crate) fn healthy() -> Self {
        Self {
            poisoned: Arc::new(AtomicBool::new(false)),
        }
    }
    pub(crate) fn poison(&self) {
        self.poisoned.store(true, Ordering::Relaxed)
    }
    pub(crate) fn poisoned(&self) -> bool {
        self.poisoned.load(Ordering::Relaxed)
    }
}
pub(super) struct Extra(Box<dyn ExtraInner>);
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Alpn {
    H2,
    None,
}
impl Connected {
    pub fn new() -> Connected {
        Connected {
            alpn: Alpn::None,
            is_proxied: false,
            extra: None,
            poisoned: PoisonPill::healthy(),
        }
    }
    pub fn proxy(mut self, is_proxied: bool) -> Connected {
        self.is_proxied = is_proxied;
        self
    }
    pub fn is_proxied(&self) -> bool {
        self.is_proxied
    }
    pub fn extra<T: Clone + Send + Sync + 'static>(mut self, extra: T) -> Connected {
        if let Some(prev) = self.extra {
            self.extra = Some(Extra(Box::new(ExtraChain(prev.0, extra))));
        } else {
            self.extra = Some(Extra(Box::new(ExtraEnvelope(extra))));
        }
        self
    }

    pub fn negotiated_h2(mut self) -> Connected {
        self.alpn = Alpn::H2;
        self
    }

    pub fn poison(&self) {
        self.poisoned.poison();
        tracing::debug ! (poison_pill = ? self . poisoned , "connection was poisoned. this connection will not be reused for subsequent requests");
    }
    pub(super) fn clone(&self) -> Connected {
        Connected {
            alpn: self.alpn,
            is_proxied: self.is_proxied,
            extra: self.extra.clone(),
            poisoned: self.poisoned.clone(),
        }
    }
}
impl Extra {
    pub(super) fn set(&self, res: &mut Extensions) {
        self.0.set(res);
    }
}
impl Clone for Extra {
    fn clone(&self) -> Extra {
        Extra(self.0.clone_box())
    }
}
impl fmt::Debug for Extra {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Extra").finish()
    }
}
trait ExtraInner: Send + Sync {
    fn clone_box(&self) -> Box<dyn ExtraInner>;
    fn set(&self, res: &mut Extensions);
}
#[derive(Clone)]
struct ExtraEnvelope<T>(T);
impl<T> ExtraInner for ExtraEnvelope<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn clone_box(&self) -> Box<dyn ExtraInner> {
        Box::new(self.clone())
    }
    fn set(&self, res: &mut Extensions) {
        res.insert(self.0.clone());
    }
}
struct ExtraChain<T>(Box<dyn ExtraInner>, T);
impl<T: Clone> Clone for ExtraChain<T> {
    fn clone(&self) -> Self {
        ExtraChain(self.0.clone_box(), self.1.clone())
    }
}
impl<T> ExtraInner for ExtraChain<T>
where
    T: Clone + Send + Sync + 'static,
{
    fn clone_box(&self) -> Box<dyn ExtraInner> {
        Box::new(self.clone())
    }
    fn set(&self, res: &mut Extensions) {
        self.0.set(res);
        res.insert(self.1.clone());
    }
}
pub(super) mod sealed {
    use super::Connection;
    use crate::http::Uri;
    use crate::hyper::rt::{Read, Write};
    use std::error::Error as StdError;
    pub trait Connect: Sealed + Sized {
        type _Svc: ConnectSvc;
        fn connect(self, internal_only: Internal, dst: Uri) -> <Self::_Svc as ConnectSvc>::Future;
    }
    pub trait ConnectSvc {
        type Connection: Read + Write + Connection + Unpin + Send + 'static;
        type Error: Into<Box<dyn StdError + Send + Sync>>;
        type Future: Future<Output = Result<Self::Connection, Self::Error>> + Unpin + Send + 'static;
        fn connect(self, internal_only: Internal, dst: Uri) -> Self::Future;
    }
    impl<S, T> Connect for S
    where
        S: crate::tower::Service<Uri, Response = T> + Send + 'static,
        S::Error: Into<Box<dyn StdError + Send + Sync>>,
        S::Future: Unpin + Send,
        T: Read + Write + Connection + Unpin + Send + 'static,
    {
        type _Svc = S;
        fn connect(self, _: Internal, dst: Uri) -> crate::hyper_util::service::Oneshot<S, Uri> {
            crate::hyper_util::service::Oneshot::new(self, dst)
        }
    }
    impl<S, T> ConnectSvc for S
    where
        S: crate::tower::Service<Uri, Response = T> + Send + 'static,
        S::Error: Into<Box<dyn StdError + Send + Sync>>,
        S::Future: Unpin + Send,
        T: Read + Write + Connection + Unpin + Send + 'static,
    {
        type Connection = T;
        type Error = S::Error;
        type Future = crate::hyper_util::service::Oneshot<S, Uri>;
        fn connect(self, _: Internal, dst: Uri) -> Self::Future {
            crate::hyper_util::service::Oneshot::new(self, dst)
        }
    }
    impl<S, T> Sealed for S
    where
        S: crate::tower::Service<Uri, Response = T> + Send,
        S::Error: Into<Box<dyn StdError + Send + Sync>>,
        S::Future: Unpin + Send,
        T: Read + Write + Connection + Unpin + Send + 'static,
    {
    }
    pub trait Sealed {}
    pub struct Internal;
}
