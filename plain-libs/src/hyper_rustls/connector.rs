use crate::http::Uri;
use crate::hyper::rt;
use crate::hyper_rustls::stream::MaybeHttpsStream;
use crate::hyper_util::client::legacy::connect::Connection;
use crate::hyper_util::rt::TokioIo;
use rustls::pki_types::ServerName;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::{fmt, io};
use tokio_rustls::TlsConnector;
use crate::tower::Service;
pub(crate) type BoxError = Box<dyn std::error::Error + Send + Sync>;
#[derive(Clone)]
pub struct HttpsConnector<T> {
    force_https: bool,
    http: T,
    tls_config: Arc<rustls::ClientConfig>,
    server_name_resolver: Arc<dyn ResolveServerName + Sync + Send>,
}
impl<T> HttpsConnector<T> {
    pub fn new(
        http: T,
        tls_config: impl Into<Arc<rustls::ClientConfig>>,
        force_https: bool,
        server_name_resolver: Arc<dyn ResolveServerName + Send + Sync>,
    ) -> Self {
        Self {
            http,
            tls_config: tls_config.into(),
            force_https,
            server_name_resolver,
        }
    }
}
impl<T> Service<Uri> for HttpsConnector<T>
where
    T: Service<Uri>,
    T::Response: Connection + rt::Read + rt::Write + Send + Unpin + 'static,
    T::Future: Send + 'static,
    T::Error: Into<BoxError>,
{
    type Response = MaybeHttpsStream<T::Response>;
    type Error = BoxError;
    type Future =
        Pin<Box<dyn Future<Output = Result<MaybeHttpsStream<T::Response>, BoxError>> + Send>>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match self.http.poll_ready(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e.into())),
            Poll::Pending => Poll::Pending,
        }
    }
    fn call(&mut self, dst: Uri) -> Self::Future {
        match dst.scheme() {
            Some(scheme) if scheme == &crate::http::uri::Scheme::HTTP && !self.force_https => {
                let future = self.http.call(dst);
                return Box::pin(async move {
                    Ok(MaybeHttpsStream::Http(future.await.map_err(Into::into)?))
                });
            }
            Some(scheme) if scheme != &crate::http::uri::Scheme::HTTPS => {
                let message = format!("unsupported scheme {scheme}");
                return Box::pin(async move { Err(io::Error::other(message).into()) });
            }
            Some(_) => {}
            None => return Box::pin(async move { Err(io::Error::other("missing scheme").into()) }),
        };
        let cfg = self.tls_config.clone();
        let hostname = match self.server_name_resolver.resolve(&dst) {
            Ok(hostname) => hostname,
            Err(e) => {
                return Box::pin(async move { Err(e) });
            }
        };
        let connecting_future = self.http.call(dst);
        Box::pin(async move {
            let tcp = connecting_future.await.map_err(Into::into)?;
            Ok(MaybeHttpsStream::Https(TokioIo::new(
                TlsConnector::from(cfg)
                    .connect(hostname, TokioIo::new(tcp))
                    .await
                    .map_err(io::Error::other)?,
            )))
        })
    }
}
impl<H, C> From<(H, C)> for HttpsConnector<H>
where
    C: Into<Arc<rustls::ClientConfig>>,
{
    fn from((http, cfg): (H, C)) -> Self {
        Self {
            force_https: false,
            http,
            tls_config: cfg.into(),
            server_name_resolver: Arc::new(DefaultServerNameResolver::default()),
        }
    }
}
impl<T> fmt::Debug for HttpsConnector<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpsConnector")
            .field("force_https", &self.force_https)
            .finish()
    }
}
#[derive(Default)]
pub struct DefaultServerNameResolver(());
impl ResolveServerName for DefaultServerNameResolver {
    fn resolve(
        &self,
        uri: &Uri,
    ) -> Result<ServerName<'static>, Box<dyn std::error::Error + Sync + Send>> {
        let mut hostname = uri.host().unwrap_or_default();
        if let Some(trimmed) = hostname.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
            hostname = trimmed;
        }
        ServerName::try_from(hostname.to_string()).map_err(|e| Box::new(e) as _)
    }
}
pub struct FixedServerNameResolver {
    name: ServerName<'static>,
}
impl FixedServerNameResolver {
    pub fn new(name: ServerName<'static>) -> Self {
        Self { name }
    }
}
impl ResolveServerName for FixedServerNameResolver {
    fn resolve(
        &self,
        _: &Uri,
    ) -> Result<ServerName<'static>, Box<dyn std::error::Error + Sync + Send>> {
        Ok(self.name.clone())
    }
}
impl<F, E> ResolveServerName for F
where
    F: Fn(&Uri) -> Result<ServerName<'static>, E>,
    E: Into<Box<dyn std::error::Error + Sync + Send>>,
{
    fn resolve(
        &self,
        uri: &Uri,
    ) -> Result<ServerName<'static>, Box<dyn std::error::Error + Sync + Send>> {
        self(uri).map_err(Into::into)
    }
}
pub trait ResolveServerName {
    fn resolve(
        &self,
        uri: &Uri,
    ) -> Result<ServerName<'static>, Box<dyn std::error::Error + Sync + Send>>;
}
