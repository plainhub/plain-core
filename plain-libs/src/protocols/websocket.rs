// HTTP upgrade adapted from axum 0.7.9 (websocket_axum/SOURCE.json).
use axum::{
    body::Body,
    extract::FromRequestParts,
    http::{HeaderMap, HeaderValue, Method, StatusCode, header, request::Parts},
    response::Response,
};
use hyper::upgrade::{OnUpgrade, Upgraded};
use hyper_util::rt::TokioIo;
use std::future::Future;
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::{
        handshake::derive_accept_key,
        protocol::{Role, WebSocketConfig},
    },
};

pub use tokio_tungstenite::tungstenite::{Message, protocol::CloseFrame};

pub type WebSocket = WebSocketStream<TokioIo<Upgraded>>;

pub struct WebSocketUpgrade {
    on_upgrade: OnUpgrade,
    key: HeaderValue,
    offered_protocol: Option<HeaderValue>,
    protocol: Option<&'static str>,
    config: WebSocketConfig,
}

impl WebSocketUpgrade {
    pub fn require_protocol(mut self, protocol: &'static str) -> Option<Self> {
        if self
            .offered_protocol
            .as_ref()
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.split(',').any(|item| item.trim() == protocol))
        {
            self.protocol = Some(protocol);
            Some(self)
        } else {
            None
        }
    }

    pub fn max_message_size(mut self, size: usize) -> Self {
        self.config.max_message_size = Some(size);
        self
    }

    pub fn max_frame_size(mut self, size: usize) -> Self {
        self.config.max_frame_size = Some(size);
        self
    }

    pub fn on_upgrade<F, Fut>(self, callback: F) -> Response
    where
        F: FnOnce(WebSocket) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let mut response = Response::builder()
            .status(StatusCode::SWITCHING_PROTOCOLS)
            .header(header::CONNECTION, "upgrade")
            .header(header::UPGRADE, "websocket")
            .header(
                header::SEC_WEBSOCKET_ACCEPT,
                derive_accept_key(self.key.as_bytes()),
            );
        if let Some(protocol) = self.protocol {
            response = response.header(header::SEC_WEBSOCKET_PROTOCOL, protocol);
        }
        tokio::spawn(async move {
            match self.on_upgrade.await {
                Ok(upgraded) => {
                    let stream = WebSocketStream::from_raw_socket(
                        TokioIo::new(upgraded),
                        Role::Server,
                        Some(self.config),
                    )
                    .await;
                    callback(stream).await;
                }
                Err(error) => log::debug!("WebSocket upgrade failed: {error}"),
            }
        });
        response
            .body(Body::empty())
            .expect("valid WebSocket response headers")
    }
}

#[axum::async_trait]
impl<S: Send + Sync> FromRequestParts<S> for WebSocketUpgrade {
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let bad_request = |message| (StatusCode::BAD_REQUEST, message);
        if parts.method != Method::GET {
            return Err((
                StatusCode::METHOD_NOT_ALLOWED,
                "Request method must be `GET`",
            ));
        }
        if !parts
            .headers
            .get_all(header::CONNECTION)
            .iter()
            .any(|value| {
                value.to_str().is_ok_and(|value| {
                    value
                        .split(',')
                        .any(|token| token.trim().eq_ignore_ascii_case("upgrade"))
                })
            })
        {
            return Err(bad_request("Connection header did not include 'upgrade'"));
        }
        if !header_eq(&parts.headers, header::UPGRADE, b"websocket") {
            return Err(bad_request("`Upgrade` header did not include 'websocket'"));
        }
        if !header_eq(&parts.headers, header::SEC_WEBSOCKET_VERSION, b"13") {
            return Err(bad_request(
                "`Sec-WebSocket-Version` header did not include '13'",
            ));
        }
        let key = parts
            .headers
            .get(header::SEC_WEBSOCKET_KEY)
            .ok_or_else(|| bad_request("`Sec-WebSocket-Key` header missing"))?;
        if key.len() != 24
            || !crate::utils::base64::base64_decode_standard(key.as_bytes())
                .is_ok_and(|key| key.len() == 16)
        {
            return Err(bad_request("Invalid `Sec-WebSocket-Key` header"));
        }
        let on_upgrade = parts.extensions.remove::<OnUpgrade>().ok_or((
            StatusCode::UPGRADE_REQUIRED,
            "WebSocket request couldn't be upgraded since no upgrade state was present",
        ))?;
        Ok(Self {
            on_upgrade,
            key: key.clone(),
            offered_protocol: parts.headers.get(header::SEC_WEBSOCKET_PROTOCOL).cloned(),
            protocol: None,
            config: WebSocketConfig::default(),
        })
    }
}

fn header_eq(headers: &HeaderMap, name: header::HeaderName, value: &[u8]) -> bool {
    headers
        .get(name)
        .is_some_and(|header| header.as_bytes().eq_ignore_ascii_case(value))
}
