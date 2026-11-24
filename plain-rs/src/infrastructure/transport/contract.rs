//! Outgoing peer-to-peer delivery over HTTPS `/peer_graphql`.
//!
//! The wire protocol (Ed25519-signed GraphQL JSON, authenticated encryption,
//! `c-id`/`c-cid` headers, encrypted response) lives here; the
//! actual HTTP POST is the app's job via the [`PeerTransport`] trait —
//! Native hosts use the shared async HTTP client.

use plain_libs::serde_json::json;

use plain_libs::utils::http_url::build_url;

use crate::infrastructure::db::DPeer;

pub(crate) const CHAT_DELIVERY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

pub struct PeerDispatch<'a> {
    pub peer: &'a DPeer,
    pub client_id: &'a str,
    pub channel_id: &'a str,
    pub key: &'a [u8],
    pub signing_key: &'a [u8],
    pub message_id: Option<&'a str>,
}

impl PeerDispatch<'_> {
    pub(crate) fn seal(&self, payload: &str) -> Result<(Vec<u8>, [u8; 32]), String> {
        let key: &[u8; 32] = self.key.try_into().map_err(|_| "Invalid peer encryption key")?;
        let sealed = if self.channel_id.is_empty() {
            super::http_request::encrypt_raw(key, "POST", "/peer_graphql", payload.as_bytes())
                .map(|body| (body, *key))
        } else {
            super::channel_request::seal(
                key, self.channel_id, self.client_id, &self.peer.id,
                self.message_id, self.signing_key, payload.as_bytes(),
            )
        };
        sealed.ok_or_else(|| "Invalid peer encryption key".to_owned())
    }
}

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
        dispatch: &PeerDispatch<'_>,
        body: &str,
    ) -> impl std::future::Future<Output = Result<(), String>> + Send {
        async move { deliver_signed(self, dispatch, body).await }
    }

    fn queued_message(
        &self,
        dispatch: &PeerDispatch<'_>,
        sign: impl Fn() -> Result<String, String> + Send + Sync,
    ) -> impl std::future::Future<Output = Result<(), String>> + Send {
        async move {
            let body = sign()?;
            tokio::time::timeout(CHAT_DELIVERY_TIMEOUT, self.message(dispatch, &body))
                .await
                .map_err(|_| "Peer delivery unconfirmed after 20s".to_string())?
        }
    }
}

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

    async fn message(&self, dispatch: &PeerDispatch<'_>, body: &str) -> Result<(), String> {
        (**self).message(dispatch, body).await
    }

    async fn queued_message(
        &self,
        dispatch: &PeerDispatch<'_>,
        sign: impl Fn() -> Result<String, String> + Send + Sync,
    ) -> Result<(), String> {
        (**self).queued_message(dispatch, sign).await
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

async fn deliver_signed<T: PeerTransport + ?Sized>(
    transport: &T,
    dispatch: &PeerDispatch<'_>,
    payload: &str,
) -> Result<(), String> {
    let (encrypted, key) = dispatch.seal(payload)?;
    let context = crate::infrastructure::transport::http_response::Context::new(&encrypted);
    let mut errors = Vec::new();

    for peer_graphql_url in peer_graphql_urls(dispatch.peer) {
        let response = match transport
            .post(&peer_graphql_url, dispatch.client_id, (!dispatch.channel_id.is_empty()).then_some(dispatch.channel_id), &encrypted)
            .await
        {
            Ok(bytes) => bytes,
            Err(e) => {
                log::warn!("deliver_to_peer request failed url={peer_graphql_url} error={e}");
                errors.push(format!("{peer_graphql_url}: {e}"));
                continue;
            }
        };
        let Some(decrypted) = context.decrypt(&key, &response) else {
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
        message_id: None,
        request_id: crate::features::record_id::generate(),
        query: query.to_owned(),
        variables: match variables {
            plain_libs::serde_json::Value::Object(map) => map,
            _ => return Err("Expected GraphQL variables object".into()),
        },
    };
    if channel_id.is_empty() {
        request.sign(kp_bytes, channel_id, None)?;
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
