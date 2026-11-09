use super::Error;
use http::{HeaderMap, HeaderValue, Uri, header, uri::Scheme};
use hyper::rt::{Read, ReadBufCursor, Write};
use hyper_rustls::{DefaultServerNameResolver, HttpsConnector, ResolveServerName};
use hyper_util::{
    client::{
        legacy::connect::{Connected, Connection, HttpConnector, proxy::Tunnel},
        proxy::matcher::Matcher,
    },
    rt::TokioIo,
};
use rustls::ClientConfig;
use std::{
    future::Future,
    io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};
use tokio_rustls::TlsConnector;
use tower::Service;

pub(super) type NativeHttp = HttpConnector<Resolver>;

pub(super) use resolver::Resolver;

mod resolver {
    use super::*;
    use hyper_util::client::legacy::connect::dns::{GaiResolver, Name};
    use std::{collections::HashMap, net::SocketAddr};

    #[derive(Clone)]
    pub(in super::super) struct Resolver {
        system: GaiResolver,
        overrides: Arc<HashMap<String, SocketAddr>>,
    }

    impl Resolver {
        pub(in super::super) fn new(overrides: HashMap<String, SocketAddr>) -> Self {
            Self {
                system: GaiResolver::new(),
                overrides: Arc::new(overrides),
            }
        }
    }

    impl Service<Name> for Resolver {
        type Response = Box<dyn Iterator<Item = SocketAddr> + Send>;
        type Error = io::Error;
        type Future = Pin<Box<dyn Future<Output = Result<Self::Response, io::Error>> + Send>>;

        fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
            self.system.poll_ready(cx)
        }

        fn call(&mut self, name: Name) -> Self::Future {
            if let Some(address) = self.overrides.get(name.as_str()).copied() {
                return Box::pin(async move {
                    Ok(Box::new(std::iter::once(address)) as Self::Response)
                });
            }
            let future = self.system.call(name);
            Box::pin(async move { Ok(Box::new(future.await?) as Self::Response) })
        }
    }
}

#[derive(Clone)]
pub(super) struct Connector {
    direct: HttpsConnector<NativeHttp>,
    proxy: HttpsConnector<NativeHttp>,
    tls: Arc<ClientConfig>,
    matcher: Option<Arc<Matcher>>,
    timeout: Option<Duration>,
    user_agent: Option<HeaderValue>,
}

impl Connector {
    pub(super) fn new(
        http: NativeHttp,
        tls: ClientConfig,
        matcher: Option<Matcher>,
        timeout: Option<Duration>,
        user_agent: Option<HeaderValue>,
    ) -> Self {
        let mut proxy_tls = tls.clone();
        proxy_tls.alpn_protocols.clear();
        let tls = Arc::new(tls);
        Self {
            direct: HttpsConnector::from((http.clone(), tls.clone())),
            proxy: HttpsConnector::from((http, Arc::new(proxy_tls))),
            tls,
            matcher: matcher.map(Arc::new),
            timeout,
            user_agent,
        }
    }

    pub(super) fn authorize_proxy(&self, dst: &Uri, headers: &mut HeaderMap) {
        if dst.scheme() == Some(&Scheme::HTTP)
            && !headers.contains_key(header::PROXY_AUTHORIZATION)
            && let Some(proxy) = self
                .matcher
                .as_ref()
                .and_then(|matcher| matcher.intercept(dst))
            && matches!(proxy.uri().scheme_str(), Some("http" | "https"))
            && let Some(auth) = proxy.basic_auth()
        {
            headers.insert(header::PROXY_AUTHORIZATION, auth.clone());
        }
    }

    async fn connect(mut self, dst: Uri) -> Result<Socket, Error> {
        let Some(proxy) = self
            .matcher
            .as_ref()
            .and_then(|matcher| matcher.intercept(&dst))
        else {
            let io = self.direct.call(dst).await.map_err(Error::connect)?;
            return Ok(Socket {
                io: Box::new(io),
                proxied: false,
            });
        };

        if dst.scheme() != Some(&Scheme::HTTPS) {
            let io = self
                .direct
                .call(proxy.uri().clone())
                .await
                .map_err(Error::connect)?;
            return Ok(Socket {
                io: Box::new(io),
                proxied: true,
            });
        }

        let mut tunnel = Tunnel::new(proxy.uri().clone(), self.proxy);
        if let Some(auth) = proxy.basic_auth() {
            tunnel = tunnel.with_auth(auth.clone());
        }
        if let Some(user_agent) = self.user_agent {
            let mut headers = HeaderMap::new();
            headers.insert(header::USER_AGENT, user_agent);
            tunnel = tunnel.with_headers(headers);
        }
        let io = tunnel.call(dst.clone()).await.map_err(Error::connect)?;
        let name = DefaultServerNameResolver::default()
            .resolve(&dst)
            .map_err(Error::connect)?;
        let io = TlsConnector::from(self.tls)
            .connect(name, TokioIo::new(io))
            .await
            .map_err(Error::connect)?;
        Ok(Socket {
            io: Box::new(TunnelTls(TokioIo::new(io))),
            proxied: false,
        })
    }
}

impl Service<Uri> for Connector {
    type Response = Socket;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Socket, Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, dst: Uri) -> Self::Future {
        let this = self.clone();
        match this.timeout {
            Some(timeout) => Box::pin(async move {
                tokio::time::timeout(timeout, this.connect(dst))
                    .await
                    .map_err(Error::connect)?
            }),
            None => Box::pin(this.connect(dst)),
        }
    }
}

type TunnelStream = TokioIo<
    tokio_rustls::client::TlsStream<
        TokioIo<hyper_rustls::MaybeHttpsStream<TokioIo<tokio::net::TcpStream>>>,
    >,
>;

struct TunnelTls(TunnelStream);

impl Connection for TunnelTls {
    fn connected(&self) -> Connected {
        self.0.inner().get_ref().0.inner().connected()
    }
}

trait Io: Read + Write + Connection + Send + Unpin {}
impl<T: Read + Write + Connection + Send + Unpin> Io for T {}

pub(super) struct Socket {
    io: Box<dyn Io>,
    proxied: bool,
}

impl Connection for Socket {
    fn connected(&self) -> Connected {
        self.io.connected().proxy(self.proxied)
    }
}

impl Read for Socket {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: ReadBufCursor<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut *self.io).poll_read(cx, buf)
    }
}

impl Write for Socket {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut *self.io).poll_write(cx, data)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut *self.io).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut *self.io).poll_shutdown(cx)
    }

    fn is_write_vectored(&self) -> bool {
        self.io.is_write_vectored()
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut *self.io).poll_write_vectored(cx, data)
    }
}

impl Read for TunnelTls {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: ReadBufCursor<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_read(cx, buf)
    }
}

impl Write for TunnelTls {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_write(cx, data)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.0).poll_shutdown(cx)
    }

    fn is_write_vectored(&self) -> bool {
        self.0.is_write_vectored()
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.0).poll_write_vectored(cx, data)
    }
}
