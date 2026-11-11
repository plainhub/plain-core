use crate::http::{HeaderMap, HeaderValue, Uri};
use crate::hyper::rt::{Read, Write};
use pin_project_lite::pin_project;
use std::error::Error as StdError;
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{self, Poll, ready};
use tower_service::Service;
#[derive(Debug, Clone)]
pub struct Tunnel<C> {
    headers: Headers,
    inner: C,
    proxy_dst: Uri,
}
#[derive(Clone, Debug)]
enum Headers {
    Empty,
    Auth(HeaderValue),
    Extra(HeaderMap),
}
#[derive(Debug)]
pub enum TunnelError {
    ConnectFailed(Box<dyn StdError + Send + Sync>),
    Io(std::io::Error),
    MissingHost,
    ProxyAuthRequired,
    ProxyHeadersTooLong,
    TunnelUnexpectedEof,
    TunnelUnsuccessful,
}
pin_project! { #[must_use = "futures do nothing unless polled"] pub struct Tunneling < F , T > { #[pin] fut : BoxTunneling < T >, _marker : PhantomData < F >, } }
type BoxTunneling<T> = Pin<Box<dyn Future<Output = Result<T, TunnelError>> + Send>>;
impl<C> Tunnel<C> {
    pub fn new(proxy_dst: Uri, connector: C) -> Self {
        Self {
            headers: Headers::Empty,
            inner: connector,
            proxy_dst,
        }
    }
    pub fn with_auth(mut self, mut auth: HeaderValue) -> Self {
        auth.set_sensitive(true);
        match self.headers {
            Headers::Empty => {
                self.headers = Headers::Auth(auth);
            }
            Headers::Auth(ref mut existing) => {
                *existing = auth;
            }
            Headers::Extra(ref mut extra) => {
                extra.insert(crate::http::header::PROXY_AUTHORIZATION, auth);
            }
        }
        self
    }
    pub fn with_headers(mut self, mut headers: HeaderMap) -> Self {
        match self.headers {
            Headers::Empty => {
                self.headers = Headers::Extra(headers);
            }
            Headers::Auth(auth) => {
                headers
                    .entry(crate::http::header::PROXY_AUTHORIZATION)
                    .or_insert(auth);
                self.headers = Headers::Extra(headers);
            }
            Headers::Extra(ref mut extra) => {
                extra.extend(headers);
            }
        }
        self
    }
}
impl<C> Service<Uri> for Tunnel<C>
where
    C: Service<Uri>,
    C::Future: Send + 'static,
    C::Response: Read + Write + Unpin + Send + 'static,
    C::Error: Into<Box<dyn StdError + Send + Sync>>,
{
    type Response = C::Response;
    type Error = TunnelError;
    type Future = Tunneling<C::Future, C::Response>;
    fn poll_ready(&mut self, cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.inner.poll_ready(cx)).map_err(|e| TunnelError::ConnectFailed(e.into()))?;
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, dst: Uri) -> Self::Future {
        let connecting = self.inner.call(self.proxy_dst.clone());
        let headers = self.headers.clone();
        Tunneling {
            fut: Box::pin(async move {
                let conn = connecting
                    .await
                    .map_err(|e| TunnelError::ConnectFailed(e.into()))?;
                tunnel(
                    conn,
                    dst.host().ok_or(TunnelError::MissingHost)?,
                    dst.port().map(|p| p.as_u16()).unwrap_or(443),
                    &headers,
                )
                .await
            }),
            _marker: PhantomData,
        }
    }
}
impl<F, T, E> Future for Tunneling<F, T>
where
    F: Future<Output = Result<T, E>>,
{
    type Output = Result<T, TunnelError>;
    fn poll(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        self.project().fut.poll(cx)
    }
}
async fn tunnel<T>(mut conn: T, host: &str, port: u16, headers: &Headers) -> Result<T, TunnelError>
where
    T: Read + Write + Unpin,
{
    let mut buf = format!(
        "\
         CONNECT {host}:{port} HTTP/1.1\r\n\
         Host: {host}:{port}\r\n\
         "
    )
    .into_bytes();
    match headers {
        Headers::Auth(auth) => {
            buf.extend_from_slice(b"Proxy-Authorization: ");
            buf.extend_from_slice(auth.as_bytes());
            buf.extend_from_slice(b"\r\n");
        }
        Headers::Extra(extra) => {
            for (name, value) in extra {
                buf.extend_from_slice(name.as_str().as_bytes());
                buf.extend_from_slice(b": ");
                buf.extend_from_slice(value.as_bytes());
                buf.extend_from_slice(b"\r\n");
            }
        }
        Headers::Empty => (),
    }
    buf.extend_from_slice(b"\r\n");
    crate::hyper_util::rt::write_all(&mut conn, &buf)
        .await
        .map_err(TunnelError::Io)?;
    let mut buf = [0; 8192];
    let mut pos = 0;
    loop {
        let n = crate::hyper_util::rt::read(&mut conn, &mut buf[pos..])
            .await
            .map_err(TunnelError::Io)?;
        if n == 0 {
            return Err(TunnelError::TunnelUnexpectedEof);
        }
        pos += n;
        let mut headers = [crate::httparse::EMPTY_HEADER; MAX_HEADERS];
        let mut res = crate::httparse::Response::new(&mut headers);
        match res.parse(&buf[..pos]) {
            Ok(crate::httparse::Status::Complete(_)) => match res.code {
                Some(200) => return Ok(conn),
                Some(407) => return Err(TunnelError::ProxyAuthRequired),
                _ => return Err(TunnelError::TunnelUnsuccessful),
            },
            Ok(crate::httparse::Status::Partial) => {
                if pos == buf.len() {
                    return Err(TunnelError::ProxyHeadersTooLong);
                }
            }
            Err(_) => return Err(TunnelError::TunnelUnsuccessful),
        }
    }
}
const MAX_HEADERS: usize = 100;
impl std::fmt::Display for TunnelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("tunnel error: ")?;
        f.write_str(match self {
            TunnelError::MissingHost => "missing destination host",
            TunnelError::ProxyAuthRequired => "proxy authorization required",
            TunnelError::ProxyHeadersTooLong => "proxy response headers too long",
            TunnelError::TunnelUnexpectedEof => "unexpected end of file",
            TunnelError::TunnelUnsuccessful => "unsuccessful",
            TunnelError::ConnectFailed(_) => "failed to create underlying connection",
            TunnelError::Io(_) => "io error establishing tunnel",
        })
    }
}
impl std::error::Error for TunnelError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TunnelError::Io(e) => Some(e),
            TunnelError::ConnectFailed(e) => Some(&**e),
            _ => None,
        }
    }
}
