use anyhow::{Result, ensure};
use futures_util::StreamExt;
use plain_rs::db::DPeer;
use plain_rs::runtime::Runtime as CoreRuntime;
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
pub struct Lan {
    pub client: reqwest::Client,
    pub capacity: Arc<tokio::sync::Semaphore>,
}
impl Lan {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .use_rustls_tls()
                .danger_accept_invalid_certs(true)
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(3))
                .build()?,
            capacity: Arc::new(tokio::sync::Semaphore::new(4)),
        })
    }
}
#[derive(Debug)]
pub enum Failure {
    Unavailable(String),
    Fatal(String),
}
pub async fn send(
    state: &CoreRuntime,
    peer: &DPeer,
    channel: &str,
    key: &[u8],
    body: &str,
    validate: impl Fn() -> Result<(), String>,
) -> Result<Value, Failure> {
    let _permit = state
        .lan
        .capacity
        .clone()
        .acquire_owned()
        .await
        .map_err(|e| Failure::Fatal(e.to_string()))?;
    validate().map_err(Failure::Fatal)?;
    let peer = plain_rs::services::peers::peer_address::current_identity(&state.db, &peer.id, peer)
        .map_err(|e| Failure::Fatal(e.to_string()))?;
    let actor = state
        .prefs
        .get::<String>(plain_rs::prefs::pref_keys::CLIENT_ID)
        .map_err(|e| Failure::Fatal(e.to_string()))?
        .filter(|id| !id.is_empty())
        .ok_or_else(|| Failure::Fatal("Missing peer actor".into()))?;
    let cipher = plain_rs::crypto::xchacha_encrypt_raw(key, body.as_bytes())
        .ok_or_else(|| Failure::Fatal("Invalid peer encryption key".into()))?;
    let context = plain_rs::transport::http_response::Context::new("/peer_graphql", &cipher);
    let response = state
        .lan
        .client
        .post(
            peer.peer_graphql_url()
                .ok_or_else(|| Failure::Unavailable("No peer address".into()))?,
        )
        .timeout(Duration::from_secs(10))
        .header("content-type", "application/octet-stream")
        .header("c-id", actor)
        .header("c-cid", channel)
        .body(cipher)
        .send()
        .await
        .map_err(|e| Failure::Unavailable(e.to_string()))?;
    let status = response.status();
    let bytes = limited(response, 4 * 1024 * 1024)
        .await
        .map_err(|e| Failure::Fatal(e.to_string()))?;
    let plain = context
        .decrypt(key, &bytes)
        .ok_or_else(|| Failure::Fatal("Failed to authenticate peer response".into()))?;
    if !status.is_success() {
        return Ok(json!({"data":null,"errors":[{"message":format!("Peer HTTP {status}")}]}));
    }
    let response: Value =
        serde_json::from_slice(&plain).map_err(|e| Failure::Fatal(e.to_string()))?;
    if !response.is_object() {
        return Err(Failure::Fatal("Peer response is not a JSON object".into()));
    }
    Ok(response)
}
pub async fn limited(response: reqwest::Response, limit: usize) -> Result<Vec<u8>> {
    ensure!(
        response.content_length().is_none_or(|n| n <= limit as u64),
        "Peer response exceeds limit"
    );
    limited_chunks(response.bytes_stream(), limit).await
}

pub async fn limited_chunks<S, E>(chunks: S, limit: usize) -> Result<Vec<u8>>
where
    S: futures_util::Stream<Item = std::result::Result<bytes::Bytes, E>>,
    E: std::error::Error + Send + Sync + 'static,
{
    futures_util::pin_mut!(chunks);
    let mut body = vec![];
    while let Some(bytes) = chunks.next().await {
        let bytes = bytes?;
        ensure!(
            body.len() + bytes.len() <= limit,
            "Peer response exceeds limit"
        );
        body.extend_from_slice(&bytes);
    }
    Ok(body)
}

pub fn incoming_chunks(
    mut body: hyper::body::Incoming,
) -> impl futures_core::Stream<Item = Result<bytes::Bytes, hyper::Error>> {
    use hyper::body::Body;
    futures_util::stream::poll_fn(move |cx| {
        loop {
            match std::pin::Pin::new(&mut body).poll_frame(cx) {
                std::task::Poll::Ready(Some(Ok(frame))) => match frame.into_data() {
                    Ok(bytes) => return std::task::Poll::Ready(Some(Ok(bytes))),
                    Err(_) => continue,
                },
                std::task::Poll::Ready(Some(Err(error))) => {
                    return std::task::Poll::Ready(Some(Err(error)));
                }
                std::task::Poll::Ready(None) => return std::task::Poll::Ready(None),
                std::task::Poll::Pending => return std::task::Poll::Pending,
            }
        }
    })
}
