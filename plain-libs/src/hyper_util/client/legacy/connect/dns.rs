pub(super) use self::sealed::Resolve;
use std::error::Error;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, ToSocketAddrs};
use std::pin::Pin;
use std::str::FromStr;
use std::task::{self, Poll};
use std::{fmt, io, vec};
use tokio::task::JoinHandle;
use crate::tower::Service;
#[derive(Clone, Hash, Eq, PartialEq)]
pub struct Name {
    host: Box<str>,
}
#[derive(Clone)]
pub struct GaiResolver {
    _priv: (),
}
pub struct GaiAddrs {
    inner: SocketAddrs,
}
pub struct GaiFuture {
    inner: JoinHandle<Result<SocketAddrs, io::Error>>,
}
impl Name {
    pub(super) fn new(host: Box<str>) -> Name {
        Name { host }
    }
    pub fn as_str(&self) -> &str {
        &self.host
    }
}
impl fmt::Debug for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.host, f)
    }
}
impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.host, f)
    }
}
impl FromStr for Name {
    type Err = InvalidNameError;
    fn from_str(host: &str) -> Result<Self, Self::Err> {
        Ok(Name::new(host.into()))
    }
}
#[derive(Debug)]
pub struct InvalidNameError(());
impl fmt::Display for InvalidNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Not a valid domain name")
    }
}
impl Error for InvalidNameError {}
impl GaiResolver {
    pub fn new() -> Self {
        GaiResolver { _priv: () }
    }
}
impl Service<Name> for GaiResolver {
    type Response = GaiAddrs;
    type Error = io::Error;
    type Future = GaiFuture;
    fn poll_ready(&mut self, _cx: &mut task::Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, name: Name) -> Self::Future {
        let blocking = tokio::task::spawn_blocking(move || {
            (&*name.host, 0)
                .to_socket_addrs()
                .map(|i| SocketAddrs { iter: i })
        });
        GaiFuture { inner: blocking }
    }
}
impl fmt::Debug for GaiResolver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("GaiResolver")
    }
}
impl Future for GaiFuture {
    type Output = Result<GaiAddrs, io::Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.inner).poll(cx).map(|res| match res {
            Ok(Ok(addrs)) => Ok(GaiAddrs { inner: addrs }),
            Ok(Err(err)) => Err(err),
            Err(join_err) => {
                if join_err.is_cancelled() {
                    Err(io::Error::new(io::ErrorKind::Interrupted, join_err))
                } else {
                    panic!("gai background task failed: {join_err:?}")
                }
            }
        })
    }
}
impl fmt::Debug for GaiFuture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("GaiFuture")
    }
}
impl Drop for GaiFuture {
    fn drop(&mut self) {
        self.inner.abort();
    }
}
impl Iterator for GaiAddrs {
    type Item = SocketAddr;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}
impl fmt::Debug for GaiAddrs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("GaiAddrs")
    }
}
pub(super) struct SocketAddrs {
    iter: vec::IntoIter<SocketAddr>,
}
impl SocketAddrs {
    pub(super) fn new(addrs: Vec<SocketAddr>) -> Self {
        SocketAddrs {
            iter: addrs.into_iter(),
        }
    }
    pub(super) fn try_parse(host: &str, port: u16) -> Option<SocketAddrs> {
        if let Ok(addr) = host.parse::<Ipv4Addr>() {
            let addr = SocketAddrV4::new(addr, port);
            return Some(SocketAddrs {
                iter: vec![SocketAddr::V4(addr)].into_iter(),
            });
        }
        if let Ok(addr) = host.parse::<Ipv6Addr>() {
            let addr = SocketAddrV6::new(addr, port, 0, 0);
            return Some(SocketAddrs {
                iter: vec![SocketAddr::V6(addr)].into_iter(),
            });
        }
        None
    }
    #[inline]
    fn filter(self, predicate: impl FnMut(&SocketAddr) -> bool) -> SocketAddrs {
        SocketAddrs::new(self.iter.filter(predicate).collect())
    }
    pub(super) fn split_by_preference(
        self,
        local_addr_ipv4: Option<Ipv4Addr>,
        local_addr_ipv6: Option<Ipv6Addr>,
    ) -> (SocketAddrs, SocketAddrs) {
        match (local_addr_ipv4, local_addr_ipv6) {
            (Some(_), None) => (self.filter(SocketAddr::is_ipv4), SocketAddrs::new(vec![])),
            (None, Some(_)) => (self.filter(SocketAddr::is_ipv6), SocketAddrs::new(vec![])),
            _ => {
                let preferring_v6 = self
                    .iter
                    .as_slice()
                    .first()
                    .map(SocketAddr::is_ipv6)
                    .unwrap_or(false);
                let (preferred, fallback) = self
                    .iter
                    .partition::<Vec<_>, _>(|addr| addr.is_ipv6() == preferring_v6);
                (SocketAddrs::new(preferred), SocketAddrs::new(fallback))
            }
        }
    }
    pub(super) fn is_empty(&self) -> bool {
        self.iter.as_slice().is_empty()
    }
    pub(super) fn len(&self) -> usize {
        self.iter.as_slice().len()
    }
}
impl Iterator for SocketAddrs {
    type Item = SocketAddr;
    #[inline]
    fn next(&mut self) -> Option<SocketAddr> {
        self.iter.next()
    }
}
mod sealed {
    use super::{Name, SocketAddr};
    use std::task::{self, Poll};
    use crate::tower::Service;
    pub trait Resolve {
        type Addrs: Iterator<Item = SocketAddr>;
        type Error: Into<Box<dyn std::error::Error + Send + Sync>>;
        type Future: Future<Output = Result<Self::Addrs, Self::Error>>;
        fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>>;
        fn resolve(&mut self, name: Name) -> Self::Future;
    }
    impl<S> Resolve for S
    where
        S: Service<Name>,
        S::Response: Iterator<Item = SocketAddr>,
        S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        type Addrs = S::Response;
        type Error = S::Error;
        type Future = S::Future;
        fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
            Service::poll_ready(self, cx)
        }
        fn resolve(&mut self, name: Name) -> Self::Future {
            Service::call(self, name)
        }
    }
}
pub(super) async fn resolve<R>(resolver: &mut R, name: Name) -> Result<R::Addrs, R::Error>
where
    R: Resolve,
{
    std::future::poll_fn(|cx| resolver.poll_ready(cx)).await?;
    resolver.resolve(name).await
}
