use plain_libs::protocols::http_client;
use rustls::{
    DigitallySignedStruct, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    pki_types::{CertificateDer, ServerName, UnixTime},
};
use std::sync::Arc;
#[derive(Debug)]
struct PeerVerifier;
impl ServerCertVerifier for PeerVerifier {
    fn verify_server_cert(
        &self,
        _: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            signature,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            signature,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}
pub fn config() -> plain_libs::anyhow::Result<rustls::ClientConfig> {
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()?
    .dangerous()
    .with_custom_certificate_verifier(Arc::new(PeerVerifier))
    .with_no_client_auth();
    Ok(config)
}
pub async fn connect(
    url: &str,
) -> plain_libs::anyhow::Result<
    plain_libs::tokio_tungstenite::WebSocketStream<plain_libs::tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
> {
    let ws_config = plain_libs::tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default()
        .max_message_size(Some(64 * 1024))
        .max_frame_size(Some(64 * 1024));
    connect_with_config(url, ws_config, None).await
}

pub async fn connect_management(
    url: &str,
) -> plain_libs::anyhow::Result<
    plain_libs::tokio_tungstenite::WebSocketStream<plain_libs::tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
> {
    connect_management_with_pin(url, None).await
}

pub(super) async fn connect_management_with_pin(
    url: &str,
    pin: Option<[u8; 32]>,
) -> plain_libs::anyhow::Result<
    plain_libs::tokio_tungstenite::WebSocketStream<plain_libs::tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
> {
    let mut url = http_client::Url::parse(url)?;
    url.set_path("/graphql");
    connect_with_config(
        url.as_str(),
        plain_libs::tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default()
            .max_message_size(Some(1024 * 1024))
            .max_frame_size(Some(1024 * 1024)),
        pin,
    )
    .await
}

async fn connect_with_config(
    url: &str,
    ws_config: plain_libs::tokio_tungstenite::tungstenite::protocol::WebSocketConfig,
    pin: Option<[u8; 32]>,
) -> plain_libs::anyhow::Result<
    plain_libs::tokio_tungstenite::WebSocketStream<plain_libs::tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
> {
    let config = match pin {
        Some(pin) => http_client::pinned_tls_config(pin)?,
        None => config()?,
    };
    let (socket, _) = plain_libs::tokio_tungstenite::connect_async_tls_with_config(
        url,
        Some(ws_config),
        false,
        Some(plain_libs::tokio_tungstenite::Connector::Rustls(Arc::new(config))),
    )
    .await?;
    Ok(socket)
}
