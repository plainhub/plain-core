pub mod upgrade;
use crate::http::{Request, Response};
use crate::http_body::Body;
use crate::hyper::server::conn::http1;
use crate::hyper::service::HttpService;
use crate::hyper::{
    body::Incoming,
    rt::{Read, ReadBuf, Timer, Write},
    service::Service,
};
use crate::hyper::{rt::bounds::Http2ServerConnExec, server::conn::http2};
use crate::hyper_util::common::rewind::Rewind;
use bytes::Bytes;
use pin_project_lite::pin_project;
use std::marker::PhantomPinned;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use std::{error::Error as StdError, io, time::Duration};
type Error = Box<dyn std::error::Error + Send + Sync>;
type Result<T> = std::result::Result<T, Error>;
const H2_PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
pub trait HttpServerConnExec<A, B: Body>: Http2ServerConnExec<A, B> {}
impl<A, B: Body, T: Http2ServerConnExec<A, B>> HttpServerConnExec<A, B> for T {}
#[derive(Clone, Debug)]
pub struct Builder<E> {
    http1: http1::Builder,
    http2: http2::Builder<E>,
    version: Option<Version>,
}
impl<E: Default> Default for Builder<E> {
    fn default() -> Self {
        Self::new(E::default())
    }
}
impl<E> Builder<E> {
    pub fn new(executor: E) -> Self {
        Self {
            http1: http1::Builder::new(),
            http2: http2::Builder::new(executor),
            version: None,
        }
    }
    pub fn http1(&mut self) -> Http1Builder<'_, E> {
        Http1Builder { inner: self }
    }
    pub fn http2(&mut self) -> Http2Builder<'_, E> {
        Http2Builder { inner: self }
    }
    pub fn http2_only(mut self) -> Self {
        assert!(self.version.is_none());
        self.version = Some(Version::H2);
        self
    }

    pub fn title_case_headers(mut self, enabled: bool) -> Self {
        self.http1.title_case_headers(enabled);
        self
    }
    pub fn preserve_header_case(mut self, enabled: bool) -> Self {
        self.http1.preserve_header_case(enabled);
        self
    }
    pub fn serve_connection<I, S, B>(&self, io: I, service: S) -> Connection<'_, I, S, E>
    where
        S: Service<Request<Incoming>, Response = Response<B>>,
        S::Future: 'static,
        S::Error: Into<Box<dyn StdError + Send + Sync>>,
        B: Body + 'static,
        B::Error: Into<Box<dyn StdError + Send + Sync>>,
        I: Read + Write + Unpin + 'static,
        E: HttpServerConnExec<S::Future, B>,
    {
        let state = match self.version {
            Some(Version::H1) => {
                let io = Rewind::new_buffered(io, Bytes::new());
                let conn = self.http1.serve_connection(io, service);
                ConnState::H1 { conn }
            }
            Some(Version::H2) => {
                let io = Rewind::new_buffered(io, Bytes::new());
                let conn = self.http2.serve_connection(io, service);
                ConnState::H2 { conn }
            }
            _ => ConnState::ReadVersion {
                read_version: read_version(io),
                builder: Cow::Borrowed(self),
                service: Some(service),
            },
        };
        Connection { state }
    }
    pub fn serve_connection_with_upgrades<I, S, B>(
        &self,
        io: I,
        service: S,
    ) -> UpgradeableConnection<'_, I, S, E>
    where
        S: Service<Request<Incoming>, Response = Response<B>>,
        S::Future: 'static,
        S::Error: Into<Box<dyn StdError + Send + Sync>>,
        B: Body + 'static,
        B::Error: Into<Box<dyn StdError + Send + Sync>>,
        I: Read + Write + Unpin + Send + 'static,
        E: HttpServerConnExec<S::Future, B>,
    {
        UpgradeableConnection {
            state: UpgradeableConnState::ReadVersion {
                read_version: read_version(io),
                builder: Cow::Borrowed(self),
                service: Some(service),
            },
        }
    }
}
#[derive(Copy, Clone, Debug)]
enum Version {
    H1,
    H2,
}
impl Version {}
fn read_version<I>(io: I) -> ReadVersion<I>
where
    I: Read + Unpin,
{
    ReadVersion {
        io: Some(io),
        buf: [MaybeUninit::uninit(); 24],
        filled: 0,
        version: Version::H2,
        canceled: false,
        _pin: PhantomPinned,
    }
}
pin_project! { struct ReadVersion < I > { io : Option < I >, buf : [MaybeUninit < u8 >; 24] , filled : usize , version : Version , canceled : bool , #[pin] _pin : PhantomPinned , } }
impl<I> ReadVersion<I> {
    pub fn cancel(self: Pin<&mut Self>) {
        *self.project().canceled = true;
    }
}
impl<I> Future for ReadVersion<I>
where
    I: Read + Unpin,
{
    type Output = io::Result<(Version, Rewind<I>)>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        if *this.canceled {
            return Poll::Ready(Err(io::Error::new(io::ErrorKind::Interrupted, "Canceled")));
        }
        let mut buf = ReadBuf::uninit(&mut *this.buf);
        unsafe {
            buf.unfilled().advance(*this.filled);
        };
        while buf.filled().len() < H2_PREFACE.len() {
            let len = buf.filled().len();
            ready!(Pin::new(this.io.as_mut().unwrap()).poll_read(cx, buf.unfilled()))?;
            *this.filled = buf.filled().len();
            if buf.filled().len() == len
                || buf.filled()[len..] != H2_PREFACE[len..buf.filled().len()]
            {
                *this.version = Version::H1;
                break;
            }
        }
        let io = this.io.take().unwrap();
        let buf = buf.filled().to_vec();
        Poll::Ready(Ok((
            *this.version,
            Rewind::new_buffered(io, Bytes::from(buf)),
        )))
    }
}
pin_project! { #[must_use = "futures do nothing unless polled"] pub struct Connection <'a , I , S , E > where S : HttpService < Incoming >, { #[pin] state : ConnState <'a , I , S , E >, } }
enum Cow<'a, T> {
    Borrowed(&'a T),
    Owned(T),
}
impl<T> std::ops::Deref for Cow<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        match self {
            Cow::Borrowed(t) => &*t,
            Cow::Owned(t) => t,
        }
    }
}
type Http1Connection<I, S> = crate::hyper::server::conn::http1::Connection<Rewind<I>, S>;
type Http2Connection<I, S, E> = crate::hyper::server::conn::http2::Connection<Rewind<I>, S, E>;
pin_project! { #[project = ConnStateProj] enum ConnState <'a , I , S , E > where S : HttpService < Incoming >, { ReadVersion { #[pin] read_version : ReadVersion < I >, builder : Cow <'a , Builder < E >>, service : Option < S >, } , H1 { #[pin] conn : Http1Connection < I , S >, } , H2 { #[pin] conn : Http2Connection < I , S , E >, } , } }
impl<I, S, E, B> Connection<'_, I, S, E>
where
    S: HttpService<Incoming, ResBody = B>,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    I: Read + Write + Unpin,
    B: Body + 'static,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
    E: HttpServerConnExec<S::Future, B>,
{
    pub fn graceful_shutdown(self: Pin<&mut Self>) {
        match self.project().state.project() {
            ConnStateProj::ReadVersion { read_version, .. } => read_version.cancel(),
            ConnStateProj::H1 { conn } => conn.graceful_shutdown(),
            ConnStateProj::H2 { conn } => conn.graceful_shutdown(),
        }
    }
    pub fn into_owned(self) -> Connection<'static, I, S, E>
    where
        Builder<E>: Clone,
    {
        Connection {
            state: match self.state {
                ConnState::ReadVersion {
                    read_version,
                    builder,
                    service,
                } => ConnState::ReadVersion {
                    read_version,
                    service,
                    builder: Cow::Owned(builder.clone()),
                },
                ConnState::H1 { conn } => ConnState::H1 { conn },
                ConnState::H2 { conn } => ConnState::H2 { conn },
            },
        }
    }
}
impl<I, S, E, B> Future for Connection<'_, I, S, E>
where
    S: Service<Request<Incoming>, Response = Response<B>>,
    S::Future: 'static,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    B: Body + 'static,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
    I: Read + Write + Unpin + 'static,
    E: HttpServerConnExec<S::Future, B>,
{
    type Output = Result<()>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            let mut this = self.as_mut().project();
            match this.state.as_mut().project() {
                ConnStateProj::ReadVersion {
                    read_version,
                    builder,
                    service,
                } => {
                    let (version, io) = ready!(read_version.poll(cx))?;
                    let service = service.take().unwrap();
                    match version {
                        Version::H1 => {
                            let conn = builder.http1.serve_connection(io, service);
                            this.state.set(ConnState::H1 { conn });
                        }
                        Version::H2 => {
                            let conn = builder.http2.serve_connection(io, service);
                            this.state.set(ConnState::H2 { conn });
                        }
                    }
                }
                ConnStateProj::H1 { conn } => {
                    return conn.poll(cx).map_err(Into::into);
                }
                ConnStateProj::H2 { conn } => {
                    return conn.poll(cx).map_err(Into::into);
                }
            }
        }
    }
}
pin_project! { #[must_use = "futures do nothing unless polled"] pub struct UpgradeableConnection <'a , I , S , E > where S : HttpService < Incoming >, { #[pin] state : UpgradeableConnState <'a , I , S , E >, } }
type Http1UpgradeableConnection<I, S> =
    crate::hyper::server::conn::http1::UpgradeableConnection<I, S>;
pin_project! { #[project = UpgradeableConnStateProj] enum UpgradeableConnState <'a , I , S , E > where S : HttpService < Incoming >, { ReadVersion { #[pin] read_version : ReadVersion < I >, builder : Cow <'a , Builder < E >>, service : Option < S >, } , H1 { #[pin] conn : Http1UpgradeableConnection < Rewind < I >, S >, } , H2 { #[pin] conn : Http2Connection < I , S , E >, } , } }
impl<I, S, E, B> UpgradeableConnection<'_, I, S, E>
where
    S: HttpService<Incoming, ResBody = B>,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    I: Read + Write + Unpin,
    B: Body + 'static,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
    E: HttpServerConnExec<S::Future, B>,
{
    pub fn graceful_shutdown(self: Pin<&mut Self>) {
        match self.project().state.project() {
            UpgradeableConnStateProj::ReadVersion { read_version, .. } => read_version.cancel(),
            UpgradeableConnStateProj::H1 { conn } => conn.graceful_shutdown(),
            UpgradeableConnStateProj::H2 { conn } => conn.graceful_shutdown(),
        }
    }
    pub fn into_owned(self) -> UpgradeableConnection<'static, I, S, E>
    where
        Builder<E>: Clone,
    {
        UpgradeableConnection {
            state: match self.state {
                UpgradeableConnState::ReadVersion {
                    read_version,
                    builder,
                    service,
                } => UpgradeableConnState::ReadVersion {
                    read_version,
                    service,
                    builder: Cow::Owned(builder.clone()),
                },
                UpgradeableConnState::H1 { conn } => UpgradeableConnState::H1 { conn },
                UpgradeableConnState::H2 { conn } => UpgradeableConnState::H2 { conn },
            },
        }
    }
}
impl<I, S, E, B> Future for UpgradeableConnection<'_, I, S, E>
where
    S: Service<Request<Incoming>, Response = Response<B>>,
    S::Future: 'static,
    S::Error: Into<Box<dyn StdError + Send + Sync>>,
    B: Body + 'static,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
    I: Read + Write + Unpin + Send + 'static,
    E: HttpServerConnExec<S::Future, B>,
{
    type Output = Result<()>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            let mut this = self.as_mut().project();
            match this.state.as_mut().project() {
                UpgradeableConnStateProj::ReadVersion {
                    read_version,
                    builder,
                    service,
                } => {
                    let (version, io) = ready!(read_version.poll(cx))?;
                    let service = service.take().unwrap();
                    match version {
                        Version::H1 => {
                            let conn = builder.http1.serve_connection(io, service).with_upgrades();
                            this.state.set(UpgradeableConnState::H1 { conn });
                        }
                        Version::H2 => {
                            let conn = builder.http2.serve_connection(io, service);
                            this.state.set(UpgradeableConnState::H2 { conn });
                        }
                    }
                }
                UpgradeableConnStateProj::H1 { conn } => {
                    return conn.poll(cx).map_err(Into::into);
                }
                UpgradeableConnStateProj::H2 { conn } => {
                    return conn.poll(cx).map_err(Into::into);
                }
            }
        }
    }
}
pub struct Http1Builder<'a, E> {
    inner: &'a mut Builder<E>,
}
impl<E> Http1Builder<'_, E> {
    pub fn http2(&mut self) -> Http2Builder<'_, E> {
        Http2Builder { inner: self.inner }
    }
    pub fn auto_date_header(&mut self, enabled: bool) -> &mut Self {
        self.inner.http1.auto_date_header(enabled);
        self
    }
    pub fn half_close(&mut self, val: bool) -> &mut Self {
        self.inner.http1.half_close(val);
        self
    }
    pub fn keep_alive(&mut self, val: bool) -> &mut Self {
        self.inner.http1.keep_alive(val);
        self
    }
    pub fn title_case_headers(&mut self, enabled: bool) -> &mut Self {
        self.inner.http1.title_case_headers(enabled);
        self
    }
    pub fn ignore_invalid_headers(&mut self, enabled: bool) -> &mut Self {
        self.inner.http1.ignore_invalid_headers(enabled);
        self
    }
    pub fn preserve_header_case(&mut self, enabled: bool) -> &mut Self {
        self.inner.http1.preserve_header_case(enabled);
        self
    }
    pub fn max_headers(&mut self, val: usize) -> &mut Self {
        self.inner.http1.max_headers(val);
        self
    }
    pub fn header_read_timeout(&mut self, read_timeout: impl Into<Option<Duration>>) -> &mut Self {
        self.inner.http1.header_read_timeout(read_timeout);
        self
    }
    pub fn writev(&mut self, val: bool) -> &mut Self {
        self.inner.http1.writev(val);
        self
    }
    pub fn max_buf_size(&mut self, max: usize) -> &mut Self {
        self.inner.http1.max_buf_size(max);
        self
    }
    pub fn pipeline_flush(&mut self, enabled: bool) -> &mut Self {
        self.inner.http1.pipeline_flush(enabled);
        self
    }
    pub fn timer<M>(&mut self, timer: M) -> &mut Self
    where
        M: Timer + Send + Sync + 'static,
    {
        self.inner.http1.timer(timer);
        self
    }
    pub async fn serve_connection<I, S, B>(&self, io: I, service: S) -> Result<()>
    where
        S: Service<Request<Incoming>, Response = Response<B>>,
        S::Future: 'static,
        S::Error: Into<Box<dyn StdError + Send + Sync>>,
        B: Body + 'static,
        B::Error: Into<Box<dyn StdError + Send + Sync>>,
        I: Read + Write + Unpin + 'static,
        E: HttpServerConnExec<S::Future, B>,
    {
        self.inner.serve_connection(io, service).await
    }
    pub fn serve_connection_with_upgrades<I, S, B>(
        &self,
        io: I,
        service: S,
    ) -> UpgradeableConnection<'_, I, S, E>
    where
        S: Service<Request<Incoming>, Response = Response<B>>,
        S::Future: 'static,
        S::Error: Into<Box<dyn StdError + Send + Sync>>,
        B: Body + 'static,
        B::Error: Into<Box<dyn StdError + Send + Sync>>,
        I: Read + Write + Unpin + Send + 'static,
        E: HttpServerConnExec<S::Future, B>,
    {
        self.inner.serve_connection_with_upgrades(io, service)
    }
}
pub struct Http2Builder<'a, E> {
    inner: &'a mut Builder<E>,
}
impl<E> Http2Builder<'_, E> {
    pub fn http1(&mut self) -> Http1Builder<'_, E> {
        Http1Builder { inner: self.inner }
    }
    pub fn max_pending_accept_reset_streams(&mut self, max: impl Into<Option<usize>>) -> &mut Self {
        self.inner.http2.max_pending_accept_reset_streams(max);
        self
    }
    pub fn max_local_error_reset_streams(&mut self, max: impl Into<Option<usize>>) -> &mut Self {
        self.inner.http2.max_local_error_reset_streams(max);
        self
    }
    pub fn initial_stream_window_size(&mut self, sz: impl Into<Option<u32>>) -> &mut Self {
        self.inner.http2.initial_stream_window_size(sz);
        self
    }
    pub fn initial_connection_window_size(&mut self, sz: impl Into<Option<u32>>) -> &mut Self {
        self.inner.http2.initial_connection_window_size(sz);
        self
    }
    pub fn adaptive_window(&mut self, enabled: bool) -> &mut Self {
        self.inner.http2.adaptive_window(enabled);
        self
    }
    pub fn max_frame_size(&mut self, sz: impl Into<Option<u32>>) -> &mut Self {
        self.inner.http2.max_frame_size(sz);
        self
    }
    pub fn max_concurrent_streams(&mut self, max: impl Into<Option<u32>>) -> &mut Self {
        self.inner.http2.max_concurrent_streams(max);
        self
    }
    pub fn keep_alive_interval(&mut self, interval: impl Into<Option<Duration>>) -> &mut Self {
        self.inner.http2.keep_alive_interval(interval);
        self
    }
    pub fn keep_alive_timeout(&mut self, timeout: Duration) -> &mut Self {
        self.inner.http2.keep_alive_timeout(timeout);
        self
    }
    pub fn max_send_buf_size(&mut self, max: usize) -> &mut Self {
        self.inner.http2.max_send_buf_size(max);
        self
    }
    pub fn enable_connect_protocol(&mut self) -> &mut Self {
        self.inner.http2.enable_connect_protocol();
        self
    }
    pub fn max_header_list_size(&mut self, max: u32) -> &mut Self {
        self.inner.http2.max_header_list_size(max);
        self
    }
    pub fn timer<M>(&mut self, timer: M) -> &mut Self
    where
        M: Timer + Send + Sync + 'static,
    {
        self.inner.http2.timer(timer);
        self
    }
    pub fn auto_date_header(&mut self, enabled: bool) -> &mut Self {
        self.inner.http2.auto_date_header(enabled);
        self
    }
    pub async fn serve_connection<I, S, B>(&self, io: I, service: S) -> Result<()>
    where
        S: Service<Request<Incoming>, Response = Response<B>>,
        S::Future: 'static,
        S::Error: Into<Box<dyn StdError + Send + Sync>>,
        B: Body + 'static,
        B::Error: Into<Box<dyn StdError + Send + Sync>>,
        I: Read + Write + Unpin + 'static,
        E: HttpServerConnExec<S::Future, B>,
    {
        self.inner.serve_connection(io, service).await
    }
    pub fn serve_connection_with_upgrades<I, S, B>(
        &self,
        io: I,
        service: S,
    ) -> UpgradeableConnection<'_, I, S, E>
    where
        S: Service<Request<Incoming>, Response = Response<B>>,
        S::Future: 'static,
        S::Error: Into<Box<dyn StdError + Send + Sync>>,
        B: Body + 'static,
        B::Error: Into<Box<dyn StdError + Send + Sync>>,
        I: Read + Write + Unpin + Send + 'static,
        E: HttpServerConnExec<S::Future, B>,
    {
        self.inner.serve_connection_with_upgrades(io, service)
    }
}
