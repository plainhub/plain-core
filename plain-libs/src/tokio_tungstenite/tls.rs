pub use crate::tokio_tungstenite::stream::MaybeTlsStream;
use crate::tokio_tungstenite::{IntoClientRequest, WebSocketStream, client_async_with_config};
use crate::tungstenite::{
    client::uri_mode, error::Error, handshake::client::Response, protocol::WebSocketConfig,
};
use tokio::io::{AsyncRead, AsyncWrite};
#[non_exhaustive]
#[derive(Clone)]
pub enum Connector {
    Plain,
    Rustls(std::sync::Arc<rustls::ClientConfig>),
}
mod encryption {
    pub mod rustls {
        use crate::tokio_tungstenite::stream::MaybeTlsStream;
        use crate::tungstenite::{Error, error::TlsError, stream::Mode};
        pub use rustls::ClientConfig;
        use rustls::RootCertStore;
        use rustls_pki_types::ServerName;
        use std::{convert::TryFrom, sync::Arc};
        use tokio::io::{AsyncRead, AsyncWrite};
        use tokio_rustls::TlsConnector as TokioTlsConnector;
        pub async fn wrap_stream<S>(
            socket: S,
            domain: String,
            mode: Mode,
            tls_connector: Option<Arc<ClientConfig>>,
        ) -> Result<MaybeTlsStream<S>, Error>
        where
            S: 'static + AsyncRead + AsyncWrite + Send + Unpin,
        {
            match mode {
                Mode::Plain => Ok(MaybeTlsStream::Plain(socket)),
                Mode::Tls => {
                    let config = match tls_connector {
                        Some(config) => config,
                        None => {
                            let mut root_store = RootCertStore::empty();
                            Arc::new(
                                ClientConfig::builder()
                                    .with_root_certificates(root_store)
                                    .with_no_client_auth(),
                            )
                        }
                    };
                    let domain = ServerName::try_from(domain.as_str())
                        .map_err(|_| TlsError::InvalidDnsName)?
                        .to_owned();
                    let stream = TokioTlsConnector::from(config);
                    let connected = stream.connect(domain, socket).await;
                    match connected {
                        Err(e) => Err(Error::Io(e)),
                        Ok(s) => Ok(MaybeTlsStream::Rustls(s)),
                    }
                }
            }
        }
    }
    pub mod plain {
        use crate::tokio_tungstenite::stream::MaybeTlsStream;
        use crate::tungstenite::{
            error::{Error, UrlError},
            stream::Mode,
        };
        use tokio::io::{AsyncRead, AsyncWrite};
        pub async fn wrap_stream<S>(socket: S, mode: Mode) -> Result<MaybeTlsStream<S>, Error>
        where
            S: 'static + AsyncRead + AsyncWrite + Send + Unpin,
        {
            match mode {
                Mode::Plain => Ok(MaybeTlsStream::Plain(socket)),
                Mode::Tls => Err(Error::Url(UrlError::TlsFeatureNotEnabled)),
            }
        }
    }
}
pub async fn client_async_tls<R, S>(
    request: R,
    stream: S,
) -> Result<(WebSocketStream<MaybeTlsStream<S>>, Response), Error>
where
    R: IntoClientRequest + Unpin,
    S: 'static + AsyncRead + AsyncWrite + Send + Unpin,
    MaybeTlsStream<S>: Unpin,
{
    client_async_tls_with_config(request, stream, None, None).await
}
pub async fn client_async_tls_with_config<R, S>(
    request: R,
    stream: S,
    config: Option<WebSocketConfig>,
    connector: Option<Connector>,
) -> Result<(WebSocketStream<MaybeTlsStream<S>>, Response), Error>
where
    R: IntoClientRequest + Unpin,
    S: 'static + AsyncRead + AsyncWrite + Send + Unpin,
    MaybeTlsStream<S>: Unpin,
{
    let request = request.into_client_request()?;
    let domain = crate::tokio_tungstenite::domain(&request)?;
    let mode = uri_mode(request.uri())?;
    let stream = match connector {
        Some(conn) => match conn {
            Connector::Rustls(conn) => {
                self::encryption::rustls::wrap_stream(stream, domain, mode, Some(conn)).await
            }
            Connector::Plain => self::encryption::plain::wrap_stream(stream, mode).await,
        },
        None => self::encryption::rustls::wrap_stream(stream, domain, mode, None).await,
    }?;
    client_async_with_config(request, stream, config).await
}
