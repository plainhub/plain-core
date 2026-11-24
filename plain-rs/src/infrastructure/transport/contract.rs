//! Outgoing peer-to-peer delivery over HTTPS `/peer_graphql`.
//!
//! The wire protocol (Ed25519-signed GraphQL JSON, XChaCha20-Poly1305
//! body, `c-id`/`c-cid` headers, encrypted response) lives here; the
//! actual HTTP POST is the app's job via the [`PeerTransport`] trait —
//! Native hosts use the shared async HTTP client.

use plain_libs::serde_json::json;

use plain_libs::crypto::ed25519_sign;
use plain_libs::utils::http_url::build_url;

use crate::infrastructure::db::DPeer;

pub(crate) const CHAT_DELIVERY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// The one transport seam of the chat stack: POST an encrypted body to a
/// peer URL with the `c-id` / `c-cid` headers and return the raw response
/// bytes. Implementations own their HTTP client, TLS policy (peers use
/// self-signed certs — certificate verification must be disabled) and
/// timeouts.
pub trait PeerTransport: Send + Sync {
    fn post<'a>(
        &'a self,
        url: &'a str,
        client_id: &'a str,
        channel_id: Option<&'a str>,
        body: &'a [u8],
    ) -> impl std::future::Future<Output = Result<Vec<u8>, String>> + Send;
    fn message(
        &self,
        peer: &DPeer,
        client_id: &str,
        channel_id: &str,
        key: &[u8],
        body: &str,
    ) -> impl std::future::Future<Output = Result<(), String>> + Send {
        async move {
            deliver_signed(
                self,
                &peer_graphql_urls(peer),
                key,
                client_id,
                body,
                (!channel_id.is_empty()).then_some(channel_id),
            )
            .await
        }
    }

    fn queued_message(
        &self,
        peer: &DPeer,
        client_id: &str,
        channel_id: &str,
        key: &[u8],
        sign: impl Fn() -> Result<String, String> + Send + Sync,
    ) -> impl std::future::Future<Output = Result<(), String>> + Send {
        async move {
            let body = sign()?;
            tokio::time::timeout(
                CHAT_DELIVERY_TIMEOUT,
                self.message(peer, client_id, channel_id, key, &body),
            )
            .await
            .map_err(|_| "Peer delivery unconfirmed after 20s".to_string())?
        }
    }
}

/// Blanket forwarding impl so callers can pass `&Arc<T>` where `&T` is
/// expected (the service holds the transport behind an Arc).
impl<T: PeerTransport> PeerTransport for std::sync::Arc<T> {
    fn post<'a>(
        &'a self,
        url: &'a str,
        client_id: &'a str,
        channel_id: Option<&'a str>,
        body: &'a [u8],
    ) -> impl std::future::Future<Output = Result<Vec<u8>, String>> + Send {
        (**self).post(url, client_id, channel_id, body)
    }
    async fn message(
        &self,
        peer: &DPeer,
        client_id: &str,
        channel_id: &str,
        key: &[u8],
        body: &str,
    ) -> Result<(), String> {
        (**self)
            .message(peer, client_id, channel_id, key, body)
            .await
    }

    async fn queued_message(
        &self,
        peer: &DPeer,
        client_id: &str,
        channel_id: &str,
        key: &[u8],
        sign: impl Fn() -> Result<String, String> + Send + Sync,
    ) -> Result<(), String> {
        (**self)
            .queued_message(peer, client_id, channel_id, key, sign)
            .await
    }
}

/// Build all candidate URLs for a peer's /peer_graphql endpoint,
/// ordered with the most-recently-seen IP first.
pub fn peer_graphql_urls(peer: &DPeer) -> Vec<String> {
    let Some(port) = peer.port.filter(|port| *port > 0) else {
        return Vec::new();
    };
    peer.ip
        .split(',')
        .map(str::trim)
        .filter(|ip| !ip.is_empty())
        .map(|ip| build_url("https", ip, port, "/peer_graphql"))
        .collect()
}

/// POST a `createChatItem` GraphQL request to `peer_graphql_urls`, trying
/// each URL in order and returning `Ok(())` on the first successful
/// delivery. Payload format: XChaCha20-Poly1305 over
/// a signed JSON envelope containing the shared UUIDv7 GraphQL request.
pub async fn deliver_to_peer<T: PeerTransport>(
    transport: &T,
    peer_graphql_urls: &[String],
    key: &[u8],
    client_id: &str,
    kp_bytes: &[u8],
    content: &str,
    channel_id: Option<&str>,
) -> Result<(), String> {
    let payload = chat_item_request(kp_bytes, content, channel_id.unwrap_or_default())?;

    deliver_signed(
        transport,
        peer_graphql_urls,
        key,
        client_id,
        &payload,
        channel_id,
    )
    .await
}

pub async fn deliver_signed<T: PeerTransport + ?Sized>(
    transport: &T,
    peer_graphql_urls: &[String],
    key: &[u8],
    client_id: &str,
    payload: &str,
    channel_id: Option<&str>,
) -> Result<(), String> {
    let Some(encrypted) = crate::infrastructure::transport::http_request::encrypt_raw(
        key,
        "POST",
        "/peer_graphql",
        payload.as_bytes(),
    ) else {
        return Err("encrypt failed".to_string());
    };

    let context = crate::infrastructure::transport::http_response::Context::new(&encrypted);
    let mut errors = Vec::new();

    for peer_graphql_url in peer_graphql_urls {
        let response = match transport
            .post(peer_graphql_url, client_id, channel_id, &encrypted)
            .await
        {
            Ok(bytes) => bytes,
            Err(e) => {
                log::warn!("deliver_to_peer request failed url={peer_graphql_url} error={e}");
                errors.push(format!("{peer_graphql_url}: {e}"));
                continue;
            }
        };
        let Some(decrypted) = context.decrypt(key, &response) else {
            log::warn!("deliver_to_peer failed decrypting response from {peer_graphql_url}");
            errors.push(format!("{peer_graphql_url}: failed decrypting response"));
            continue;
        };
        let Ok(value) =
            plain_libs::serde_json::from_slice::<plain_libs::serde_json::Value>(&decrypted)
        else {
            log::warn!("deliver_to_peer invalid JSON from {peer_graphql_url}");
            errors.push(format!("{peer_graphql_url}: invalid JSON response"));
            continue;
        };
        if let Err(error) = message_response(&value) {
            errors.push(format!("{peer_graphql_url}: {error}"));
            continue;
        }
        return Ok(());
    }

    Err(if errors.is_empty() {
        "delivery failed".to_string()
    } else {
        errors.join("; ")
    })
}

pub fn message_response(value: &plain_libs::serde_json::Value) -> Result<(), String> {
    if let Some(errors) = value.get("errors") {
        match errors {
            plain_libs::serde_json::Value::Null => {}
            plain_libs::serde_json::Value::Array(errors) if errors.is_empty() => {}
            _ => return Err(format!("GraphQL errors {errors}")),
        }
    }
    if !value
        .get("data")
        .and_then(|data| data.get("createChatItem"))
        .is_some_and(plain_libs::serde_json::Value::is_array)
    {
        return Err("Missing createChatItem delivery receipt".into());
    }
    Ok(())
}

pub const CREATE_CHAT_ITEM: &str = "mutation CreateChatItem($content: String!) { createChatItem(content: $content) { id fromId toId createdAt } }";

pub fn signed_request(
    kp_bytes: &[u8],
    query: &str,
    variables: plain_libs::serde_json::Value,
    channel_id: &str,
) -> Result<String, String> {
    let mut request = super::graphql_request::SignedEnvelope {
        signature: String::new(),
        request_id: crate::features::record_id::generate(),
        query: query.to_owned(),
        variables: match variables {
            plain_libs::serde_json::Value::Object(map) => map,
            _ => return Err("Expected GraphQL variables object".into()),
        },
    };
    let signed = request.signing_bytes(channel_id);
    request.signature = ed25519_sign(kp_bytes, &signed);
    if kp_bytes.len() != 64
        || !plain_libs::crypto::ed25519_verify(
            &plain_libs::utils::base64::base64_encode(&kp_bytes[32..]),
            &signed,
            &request.signature,
        )
    {
        return Err("Invalid peer signing keypair".into());
    }
    plain_libs::serde_json::to_string(&request).map_err(|e| e.to_string())
}
pub fn chat_item_request(
    kp_bytes: &[u8],
    content: &str,
    channel_id: &str,
) -> Result<String, String> {
    signed_request(
        kp_bytes,
        CREATE_CHAT_ITEM,
        json!({"content":content}),
        channel_id,
    )
}
