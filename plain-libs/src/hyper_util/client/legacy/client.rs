use super::connect::HttpConnector;
use super::connect::capture::CaptureConnectionExtension;
use super::connect::{Alpn, Connect, Connected, Connection};
use super::pool::{self, Ver};
use crate::http::uri::Scheme;
use crate::hyper::client::conn::TrySendError as ConnTrySendError;
use crate::hyper::header::{HOST, HeaderValue};
use crate::hyper::rt::Timer;
use crate::hyper::{Method, Request, Response, Uri, Version, body::Body};
use crate::hyper_util::common::{Exec, Lazy, SyncWrapper, lazy as hyper_lazy, timer};
use futures_util::future::{self, Either, FutureExt, TryFutureExt};
use std::error::Error as StdError;
use std::fmt;
use std::future::poll_fn;
use std::pin::Pin;
use std::task::{self, Poll};
use std::time::Duration;
use tracing::{debug, trace, warn};
type BoxSendFuture = Pin<Box<dyn Future<Output = ()> + Send>>;
pub struct Client<C, B> {
    config: Config,
    connector: C,
    exec: Exec,
    h1_builder: crate::hyper::client::conn::http1::Builder,
    h2_builder: crate::hyper::client::conn::http2::Builder<Exec>,
    pool: pool::Pool<PoolClient<B>, PoolKey>,
}
#[derive(Clone, Copy, Debug)]
struct Config {
    retry_canceled_requests: bool,
    set_host: bool,
    ver: Ver,
}
pub struct Error {
    kind: ErrorKind,
    source: Option<Box<dyn StdError + Send + Sync>>,
    connect_info: Option<Connected>,
}
#[derive(Debug)]
enum ErrorKind {
    Canceled,
    ChannelClosed,
    Connect,
    UserUnsupportedRequestMethod,
    UserUnsupportedVersion,
    UserAbsoluteUriRequired,
    SendRequest,
}
macro_rules! e {
    ($kind : ident) => {
        Error {
            kind: ErrorKind::$kind,
            source: None,
            connect_info: None,
        }
    };
    ($kind : ident , $src : expr) => {
        Error {
            kind: ErrorKind::$kind,
            source: Some($src.into()),
            connect_info: None,
        }
    };
}
type PoolKey = (crate::http::uri::Scheme, crate::http::uri::Authority);
enum TrySendError<B> {
    Retryable {
        error: Error,
        req: Request<B>,
        connection_reused: bool,
    },
    Nope(Error),
}
#[must_use = "futures do nothing unless polled"]
pub struct ResponseFuture {
    inner: SyncWrapper<
        Pin<Box<dyn Future<Output = Result<Response<crate::hyper::body::Incoming>, Error>> + Send>>,
    >,
}
impl Client<(), ()> {
    pub fn builder<E>(executor: E) -> Builder
    where
        E: crate::hyper::rt::Executor<BoxSendFuture> + Send + Sync + Clone + 'static,
    {
        Builder::new(executor)
    }
}
impl<C, B> Client<C, B>
where
    C: Connect + Clone + Send + Sync + 'static,
    B: Body + Send + 'static + Unpin,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    pub fn get(&self, uri: Uri) -> ResponseFuture
    where
        B: Default,
    {
        let body = B::default();
        if !body.is_end_stream() {
            warn!("default Body used for get() does not return true for is_end_stream");
        }
        let mut req = Request::new(body);
        *req.uri_mut() = uri;
        self.request(req)
    }
    pub fn request(&self, mut req: Request<B>) -> ResponseFuture {
        let is_http_connect = req.method() == Method::CONNECT;
        match req.version() {
            Version::HTTP_11 => (),
            Version::HTTP_10 => {
                if is_http_connect {
                    warn!("CONNECT is not allowed for HTTP/1.0");
                    return ResponseFuture::new(future::err(e!(UserUnsupportedRequestMethod)));
                }
            }
            Version::HTTP_2 => (),
            other => return ResponseFuture::error_version(other),
        };
        let pool_key = match extract_domain(req.uri_mut(), is_http_connect) {
            Ok(s) => s,
            Err(err) => {
                return ResponseFuture::new(future::err(err));
            }
        };
        ResponseFuture::new(self.clone().send_request(req, pool_key))
    }
    async fn send_request(
        self,
        mut req: Request<B>,
        pool_key: PoolKey,
    ) -> Result<Response<crate::hyper::body::Incoming>, Error> {
        let uri = req.uri().clone();
        loop {
            req = match self.try_send_request(req, pool_key.clone()).await {
                Ok(resp) => return Ok(resp),
                Err(TrySendError::Nope(err)) => return Err(err),
                Err(TrySendError::Retryable {
                    mut req,
                    error,
                    connection_reused,
                }) => {
                    if !self.config.retry_canceled_requests || !connection_reused {
                        return Err(error);
                    }
                    trace!(
                        "unstarted request canceled, trying again (reason={:?})",
                        error
                    );
                    *req.uri_mut() = uri.clone();
                    req
                }
            }
        }
    }
    async fn try_send_request(
        &self,
        mut req: Request<B>,
        pool_key: PoolKey,
    ) -> Result<Response<crate::hyper::body::Incoming>, TrySendError<B>> {
        let mut pooled = self
            .connection_for(pool_key)
            .await
            .map_err(TrySendError::Nope)?;
        if let Some(conn) = req.extensions_mut().get_mut::<CaptureConnectionExtension>() {
            conn.set(&pooled.conn_info);
        }
        if pooled.is_http1() {
            if req.version() == Version::HTTP_2 {
                warn!("Connection is HTTP/1, but request requires HTTP/2");
                return Err(TrySendError::Nope(
                    e!(UserUnsupportedVersion).with_connect_info(pooled.conn_info.clone()),
                ));
            }
            if self.config.set_host {
                let uri = req.uri().clone();
                req.headers_mut().entry(HOST).or_insert_with(|| {
                    let hostname = uri.host().expect("authority implies host");
                    if let Some(port) = get_non_default_port(&uri) {
                        let s = format!("{hostname}:{port}");
                        HeaderValue::from_maybe_shared(bytes::Bytes::from(s))
                    } else {
                        HeaderValue::from_str(hostname)
                    }
                    .expect("uri host is valid header value")
                });
            }
            if req.method() == Method::CONNECT {
                authority_form(req.uri_mut());
            } else if pooled.conn_info.is_proxied {
                absolute_form(req.uri_mut());
            } else {
                origin_form(req.uri_mut());
            }
        } else if req.method() == Method::CONNECT && !pooled.is_http2() {
            authority_form(req.uri_mut());
        }
        let mut res = match pooled.try_send_request(req).await {
            Ok(res) => res,
            Err(mut err) => {
                return if let Some(req) = err.take_message() {
                    Err(TrySendError::Retryable {
                        connection_reused: pooled.is_reused(),
                        error: e!(Canceled, err.into_error())
                            .with_connect_info(pooled.conn_info.clone()),
                        req,
                    })
                } else {
                    Err(TrySendError::Nope(
                        e!(SendRequest, err.into_error())
                            .with_connect_info(pooled.conn_info.clone()),
                    ))
                };
            }
        };
        if let Some(extra) = &pooled.conn_info.extra {
            extra.set(res.extensions_mut());
        }
        if pooled.is_http2() || !pooled.is_pool_enabled() || pooled.is_ready() {
            drop(pooled);
        } else {
            let on_idle = poll_fn(move |cx| pooled.poll_ready(cx)).map(|_| ());
            self.exec.execute(on_idle);
        }
        Ok(res)
    }
    async fn connection_for(
        &self,
        pool_key: PoolKey,
    ) -> Result<pool::Pooled<PoolClient<B>, PoolKey>, Error> {
        loop {
            match self.one_connection_for(pool_key.clone()).await {
                Ok(pooled) => return Ok(pooled),
                Err(ClientConnectError::Normal(err)) => return Err(err),
                Err(ClientConnectError::CheckoutIsClosed(reason)) => {
                    if !self.config.retry_canceled_requests {
                        return Err(e!(Connect, reason));
                    }
                    trace!(
                        "unstarted request canceled, trying again (reason={:?})",
                        reason,
                    );
                    continue;
                }
            };
        }
    }
    async fn one_connection_for(
        &self,
        pool_key: PoolKey,
    ) -> Result<pool::Pooled<PoolClient<B>, PoolKey>, ClientConnectError> {
        if !self.pool.is_enabled() {
            return self
                .connect_to(pool_key)
                .await
                .map_err(ClientConnectError::Normal);
        }
        let checkout = self.pool.checkout(pool_key.clone());
        let connect = self.connect_to(pool_key);
        let is_ver_h2 = self.config.ver == Ver::Http2;
        match future::select(checkout, connect).await {
            Either::Left((Ok(checked_out), connecting)) => {
                if connecting.started() {
                    let bg = connecting
                        .map_err(|err| {
                            trace!("background connect error: {}", err);
                        })
                        .map(|_pooled| {});
                    self.exec.execute(bg);
                }
                Ok(checked_out)
            }
            Either::Right((Ok(connected), _checkout)) => Ok(connected),
            Either::Left((Err(err), connecting)) => {
                if err.is_canceled() {
                    connecting.await.map_err(ClientConnectError::Normal)
                } else {
                    Err(ClientConnectError::Normal(e!(Connect, err)))
                }
            }
            Either::Right((Err(err), checkout)) => {
                if err.is_canceled() {
                    checkout.await.map_err(move |err| {
                        if is_ver_h2 && err.is_canceled() {
                            ClientConnectError::CheckoutIsClosed(err)
                        } else {
                            ClientConnectError::Normal(e!(Connect, err))
                        }
                    })
                } else {
                    Err(ClientConnectError::Normal(err))
                }
            }
        }
    }
    fn connect_to(
        &self,
        pool_key: PoolKey,
    ) -> impl Lazy<Output = Result<pool::Pooled<PoolClient<B>, PoolKey>, Error>> + Send + Unpin + use<C, B>
    {
        let executor = self.exec.clone();
        let pool = self.pool.clone();
        let h1_builder = self.h1_builder.clone();
        let h2_builder = self.h2_builder.clone();
        let ver = self.config.ver;
        let is_ver_h2 = ver == Ver::Http2;
        let connector = self.connector.clone();
        hyper_lazy(move || {
            let connecting = match pool.connecting(&pool_key, ver) {
                Some(lock) => lock,
                None => {
                    let canceled = e!(Canceled);
                    return Either::Right(future::err(canceled));
                }
            };
            let dst = domain_as_uri(pool_key);
            Either::Left (connector . connect (super::connect::sealed::Internal , dst) . map_err (| src | e ! (Connect , src)) . and_then (move | io | { let connected = io . connected () ; let connecting = if connected . alpn == Alpn::H2 && ! is_ver_h2 { match connecting . alpn_h2 (& pool) { Some (lock) => { trace ! ("ALPN negotiated h2, updating pool") ; lock } None => { let canceled = e ! (Canceled , "ALPN upgraded to HTTP/2") ; return Either::Right (future::err (canceled)) ; } } } else { connecting } ; let is_h2 = is_ver_h2 || connected . alpn == Alpn::H2 ; Either::Left (Box::pin (async move { let tx = if is_h2 { { let (mut tx , conn) = h2_builder . handshake (io) . await . map_err (Error::tx) ? ; trace ! ("http2 handshake complete, spawning background dispatcher task") ; executor . execute (conn . map_err (| e | debug ! ("client connection error: {}" , e)) . map (| _ | ()) ,) ; tx . ready () . await . map_err (Error::tx) ? ; PoolTx::Http2 (tx) } } else { { let (mut tx , conn) = h1_builder . handshake (io) . await . map_err (crate::hyper_util::client::legacy::client::Error::tx) ? ; trace ! ("http1 handshake complete, spawning background dispatcher task") ; let (err_tx , err_rx) = tokio::sync::oneshot::channel () ; executor . execute (conn . with_upgrades () . map_err (| e | { debug ! ("client connection error: {:?}" , e) ; trace ! ("sending connection error to error channel") ; let _ = err_tx . send (e) ; }) . map (| _ | ()) ,) ; trace ! ("waiting for connection to be ready") ; match tx . ready () . await { Ok (_) => { trace ! ("connection is ready") ; drop (err_rx) ; PoolTx::Http1 (tx) } Err (e) if e . is_closed () => { trace ! ("connection channel closed, checking for connection error") ; match err_rx . await { Ok (err) => { trace ! ("received connection error: {:?}" , err) ; return Err (crate::hyper_util::client::legacy::client::Error::tx (err)) ; } Err (_) => { trace ! ("error channel closed, returning the vague ChannelClosed error") ; return Err (crate::hyper_util::client::legacy::client::Error::tx (e)) ; } } } Err (e) => { trace ! ("connection readiness failed: {:?}" , e) ; return Err (crate::hyper_util::client::legacy::client::Error::tx (e)) ; } } } } ; Ok (pool . pooled (connecting , PoolClient { conn_info : connected , tx } ,)) })) }) ,)
        })
    }
}
impl<C, B> tower_service::Service<Request<B>> for Client<C, B>
where
    C: Connect + Clone + Send + Sync + 'static,
    B: Body + Send + 'static + Unpin,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    type Response = Response<crate::hyper::body::Incoming>;
    type Error = Error;
    type Future = ResponseFuture;
    fn poll_ready(&mut self, _: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, req: Request<B>) -> Self::Future {
        self.request(req)
    }
}
impl<C, B> tower_service::Service<Request<B>> for &'_ Client<C, B>
where
    C: Connect + Clone + Send + Sync + 'static,
    B: Body + Send + 'static + Unpin,
    B::Data: Send,
    B::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    type Response = Response<crate::hyper::body::Incoming>;
    type Error = Error;
    type Future = ResponseFuture;
    fn poll_ready(&mut self, _: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, req: Request<B>) -> Self::Future {
        self.request(req)
    }
}
impl<C: Clone, B> Clone for Client<C, B> {
    fn clone(&self) -> Client<C, B> {
        Client {
            config: self.config,
            exec: self.exec.clone(),
            h1_builder: self.h1_builder.clone(),
            h2_builder: self.h2_builder.clone(),
            connector: self.connector.clone(),
            pool: self.pool.clone(),
        }
    }
}
impl<C, B> fmt::Debug for Client<C, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client").finish()
    }
}
impl ResponseFuture {
    fn new<F>(value: F) -> Self
    where
        F: Future<Output = Result<Response<crate::hyper::body::Incoming>, Error>> + Send + 'static,
    {
        Self {
            inner: SyncWrapper::new(Box::pin(value)),
        }
    }
    fn error_version(ver: Version) -> Self {
        warn!("Request has unsupported version \"{:?}\"", ver);
        ResponseFuture::new(Box::pin(future::err(e!(UserUnsupportedVersion))))
    }
}
impl fmt::Debug for ResponseFuture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("Future<Response>")
    }
}
impl Future for ResponseFuture {
    type Output = Result<Response<crate::hyper::body::Incoming>, Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        self.inner.get_mut().as_mut().poll(cx)
    }
}
struct PoolClient<B> {
    conn_info: Connected,
    tx: PoolTx<B>,
}
enum PoolTx<B> {
    Http1(crate::hyper::client::conn::http1::SendRequest<B>),
    Http2(crate::hyper::client::conn::http2::SendRequest<B>),
}
impl<B> PoolClient<B> {
    fn poll_ready(
        &mut self,
        #[allow(unused_variables)] cx: &mut task::Context<'_>,
    ) -> Poll<Result<(), Error>> {
        match self.tx {
            PoolTx::Http1(ref mut tx) => tx.poll_ready(cx).map_err(Error::closed),
            PoolTx::Http2(_) => Poll::Ready(Ok(())),
        }
    }
    fn is_http1(&self) -> bool {
        !self.is_http2()
    }
    fn is_http2(&self) -> bool {
        match self.tx {
            PoolTx::Http1(_) => false,
            PoolTx::Http2(_) => true,
        }
    }
    fn is_poisoned(&self) -> bool {
        self.conn_info.poisoned.poisoned()
    }
    fn is_ready(&self) -> bool {
        match self.tx {
            PoolTx::Http1(ref tx) => tx.is_ready(),
            PoolTx::Http2(ref tx) => tx.is_ready(),
        }
    }
}
impl<B: Body + 'static> PoolClient<B> {
    fn try_send_request(
        &mut self,
        req: Request<B>,
    ) -> impl Future<Output = Result<Response<crate::hyper::body::Incoming>, ConnTrySendError<Request<B>>>>
    where
        B: Send,
    {
        return match self.tx {
            PoolTx::Http1(ref mut tx) => Either::Left(tx.try_send_request(req)),
            PoolTx::Http2(ref mut tx) => Either::Right(tx.try_send_request(req)),
        };
    }
}
impl<B> pool::Poolable for PoolClient<B>
where
    B: Send + 'static,
{
    fn is_open(&self) -> bool {
        !self.is_poisoned() && self.is_ready()
    }
    fn reserve(self) -> pool::Reservation<Self> {
        match self.tx {
            PoolTx::Http1(tx) => pool::Reservation::Unique(PoolClient {
                conn_info: self.conn_info,
                tx: PoolTx::Http1(tx),
            }),
            PoolTx::Http2(tx) => {
                let b = PoolClient {
                    conn_info: self.conn_info.clone(),
                    tx: PoolTx::Http2(tx.clone()),
                };
                let a = PoolClient {
                    conn_info: self.conn_info,
                    tx: PoolTx::Http2(tx),
                };
                pool::Reservation::Shared(a, b)
            }
        }
    }
    fn can_share(&self) -> bool {
        self.is_http2()
    }
}
enum ClientConnectError {
    Normal(Error),
    CheckoutIsClosed(pool::Error),
}
fn origin_form(uri: &mut Uri) {
    let path = match uri.path_and_query() {
        Some(path) if path.as_str() != "/" => {
            let mut parts = crate::http::uri::Parts::default();
            parts.path_and_query = Some(path.clone());
            Uri::from_parts(parts).expect("path is valid uri")
        }
        _none_or_just_slash => {
            debug_assert!(Uri::default() == "/");
            Uri::default()
        }
    };
    *uri = path
}
fn absolute_form(uri: &mut Uri) {
    debug_assert!(uri.scheme().is_some(), "absolute_form needs a scheme");
    debug_assert!(
        uri.authority().is_some(),
        "absolute_form needs an authority"
    );
}
fn authority_form(uri: &mut Uri) {
    if let Some(path) = uri.path_and_query() {
        if path != "/" {
            warn!("HTTP/1.1 CONNECT request stripping path: {:?}", path);
        }
    }
    *uri = match uri.authority() {
        Some(auth) => {
            let mut parts = crate::http::uri::Parts::default();
            parts.authority = Some(auth.clone());
            Uri::from_parts(parts).expect("authority is valid")
        }
        None => {
            unreachable!("authority_form with relative uri");
        }
    };
}
fn extract_domain(uri: &mut Uri, is_http_connect: bool) -> Result<PoolKey, Error> {
    let uri_clone = uri.clone();
    match (uri_clone.scheme(), uri_clone.authority()) {
        (Some(scheme), Some(auth)) => Ok((scheme.clone(), auth.clone())),
        (None, Some(auth)) if is_http_connect => {
            let scheme = match auth.port_u16() {
                Some(443) => {
                    set_scheme(uri, Scheme::HTTPS);
                    Scheme::HTTPS
                }
                _ => {
                    set_scheme(uri, Scheme::HTTP);
                    Scheme::HTTP
                }
            };
            Ok((scheme, auth.clone()))
        }
        _ => {
            debug!("Client requires absolute-form URIs, received: {:?}", uri);
            Err(e!(UserAbsoluteUriRequired))
        }
    }
}
fn domain_as_uri((scheme, auth): PoolKey) -> Uri {
    crate::http::uri::Builder::new()
        .scheme(scheme)
        .authority(auth)
        .path_and_query("/")
        .build()
        .expect("domain is valid Uri")
}
fn set_scheme(uri: &mut Uri, scheme: Scheme) {
    debug_assert!(
        uri.scheme().is_none(),
        "set_scheme expects no existing scheme"
    );
    let old = std::mem::take(uri);
    let mut parts: crate::http::uri::Parts = old.into();
    parts.scheme = Some(scheme);
    parts.path_and_query = Some("/".parse().expect("slash is a valid path"));
    *uri = Uri::from_parts(parts).expect("scheme is valid");
}
fn get_non_default_port(uri: &Uri) -> Option<crate::http::uri::Port<&str>> {
    match (uri.port().map(|p| p.as_u16()), is_schema_secure(uri)) {
        (Some(443), true) => None,
        (Some(80), false) => None,
        _ => uri.port(),
    }
}
fn is_schema_secure(uri: &Uri) -> bool {
    uri.scheme_str()
        .map(|scheme_str| matches!(scheme_str, "wss" | "https"))
        .unwrap_or_default()
}
#[derive(Clone)]
pub struct Builder {
    client_config: Config,
    exec: Exec,
    h1_builder: crate::hyper::client::conn::http1::Builder,
    h2_builder: crate::hyper::client::conn::http2::Builder<Exec>,
    pool_config: pool::Config,
    pool_timer: Option<timer::Timer>,
}
impl Builder {
    pub fn new<E>(executor: E) -> Self
    where
        E: crate::hyper::rt::Executor<BoxSendFuture> + Send + Sync + Clone + 'static,
    {
        let exec = Exec::new(executor);
        Self {
            client_config: Config {
                retry_canceled_requests: true,
                set_host: true,
                ver: Ver::Auto,
            },
            exec: exec.clone(),
            h1_builder: crate::hyper::client::conn::http1::Builder::new(),
            h2_builder: crate::hyper::client::conn::http2::Builder::new(exec),
            pool_config: pool::Config {
                idle_timeout: Some(Duration::from_secs(90)),
                max_idle_per_host: usize::MAX,
            },
            pool_timer: None,
        }
    }
    pub fn pool_idle_timeout<D>(&mut self, val: D) -> &mut Self
    where
        D: Into<Option<Duration>>,
    {
        self.pool_config.idle_timeout = val.into();
        self
    }
    pub fn pool_max_idle_per_host(&mut self, max_idle: usize) -> &mut Self {
        self.pool_config.max_idle_per_host = max_idle;
        self
    }

    pub fn http09_responses(&mut self, val: bool) -> &mut Self {
        self.h1_builder.http09_responses(val);
        self
    }
    pub fn http2_only(&mut self, val: bool) -> &mut Self {
        self.client_config.ver = if val { Ver::Http2 } else { Ver::Auto };
        self
    }

    pub fn timer<M>(&mut self, timer: M) -> &mut Self
    where
        M: Timer + Send + Sync + 'static,
    {
        self.h2_builder.timer(timer);
        self
    }
    pub fn pool_timer<M>(&mut self, timer: M) -> &mut Self
    where
        M: Timer + Clone + Send + Sync + 'static,
    {
        self.pool_timer = Some(timer::Timer::new(timer.clone()));
        self
    }

    #[inline]
    pub fn retry_canceled_requests(&mut self, val: bool) -> &mut Self {
        self.client_config.retry_canceled_requests = val;
        self
    }
    #[inline]
    pub fn set_host(&mut self, val: bool) -> &mut Self {
        self.client_config.set_host = val;
        self
    }

    pub fn build<C, B>(&self, connector: C) -> Client<C, B>
    where
        C: Connect + Clone,
        B: Body + Send,
        B::Data: Send,
    {
        let exec = self.exec.clone();
        let timer = self.pool_timer.clone();
        Client {
            config: self.client_config,
            exec: exec.clone(),
            h1_builder: self.h1_builder.clone(),
            h2_builder: self.h2_builder.clone(),
            connector,
            pool: pool::Pool::new(self.pool_config, exec, timer),
        }
    }
}
impl fmt::Debug for Builder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Builder")
            .field("client_config", &self.client_config)
            .field("pool_config", &self.pool_config)
            .finish()
    }
}
impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut f = f.debug_tuple("crate::hyper_util::client::legacy::Error");
        f.field(&self.kind);
        if let Some(ref cause) = self.source {
            f.field(cause);
        }
        f.finish()
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "client error ({:?})", self.kind)
    }
}
impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source.as_ref().map(|e| &**e as _)
    }
}
impl Error {
    pub fn is_connect(&self) -> bool {
        matches!(self.kind, ErrorKind::Connect)
    }
    pub fn connect_info(&self) -> Option<&Connected> {
        self.connect_info.as_ref()
    }
    fn with_connect_info(self, connect_info: Connected) -> Self {
        Self {
            connect_info: Some(connect_info),
            ..self
        }
    }
    fn is_canceled(&self) -> bool {
        matches!(self.kind, ErrorKind::Canceled)
    }
    fn tx(src: crate::hyper::Error) -> Self {
        e!(SendRequest, src)
    }
    fn closed(src: crate::hyper::Error) -> Self {
        e!(ChannelClosed, src)
    }
}
