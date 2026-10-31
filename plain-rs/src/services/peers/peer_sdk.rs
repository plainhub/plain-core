use anyhow::{Result, ensure};
use plain_rs::host::bridge::Host;
use plain_rs::runtime::Runtime as CoreRuntime;
use plain_rs::services::peers::peer_lan::Failure;
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};

pub struct Lease {
    host: Arc<Host>,
    token: String,
    jobs: Vec<tokio::task::JoinHandle<()>>,
}
impl Drop for Lease {
    fn drop(&mut self) {
        for job in &self.jobs {
            job.abort();
        }
        let host = self.host.clone();
        let token = self.token.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                let _ = host
                    .call("peerTransportSocketClose", json!({"token":token}))
                    .await;
            });
        }
    }
}
async fn socket(state: &CoreRuntime, peer: &plain_rs::db::DPeer) -> Result<(DuplexStream, Lease)> {
    ensure!(
        state.aware.subscribe(state, &peer.id).await?,
        "Aware unavailable"
    );
    let token = plain_rs::utils::short_uuid::short_uuid();
    let mut lease = Lease {
        host: state.host.clone(),
        token: token.clone(),
        jobs: vec![],
    };
    let value = state
        .host
        .call(
            "peerTransportSocketOpen",
            json!({"token":token,"peerId":peer.id}),
        )
        .await
        .map_err(anyhow::Error::msg)?;
    ensure!(value == json!(true), "Aware socket unavailable");
    let (io, pump) = tokio::io::duplex(64 * 1024);
    let (mut reader, mut writer) = tokio::io::split(pump);
    let host = state.host.clone();
    let read_token = token.clone();
    let mut stop = state.stop.clone();
    lease.jobs.push(tokio::spawn(async move {
        loop {
            let response=tokio::select! {_=stop.changed()=>break,response=host.call("peerTransportSocketRead",json!({"token":read_token,"length":32768}))=>response};
            let Ok(response)=response else {break};
            if response["eof"]==true {break;}
            if let Some(encoded)=response["bytes"].as_str() {
                let bytes=plain_rs::utils::base64::base64_decode(encoded);
                if bytes.len()>32768 || writer.write_all(&bytes).await.is_err() {break;}
            }
        }
    }));
    let host = state.host.clone();
    let mut stop = state.stop.clone();
    lease.jobs.push(tokio::spawn(async move {
        let mut bytes = vec![0; 32768];
        loop {
            let n = tokio::select! {_=stop.changed()=>break,n=reader.read(&mut bytes)=>n};
            let Ok(n) = n else { break };
            if n == 0 {
                break;
            }
            let result = host
                .call(
                    "peerTransportSocketWrite",
                    json!({"token":token,"bytes":plain_rs::utils::base64::base64_encode(&bytes[..n])}),
                )
                .await;
            if !matches!(result,Ok(value) if value==json!(n)) {
                break;
            }
        }
    }));
    Ok((io, lease))
}
fn aware_request(
    method: &str,
    path: &str,
    actor: &str,
    channel: &str,
    body: Vec<u8>,
) -> Result<hyper::Request<RequestBody>> {
    Ok(hyper::Request::builder()
        .method(method)
        .uri(path)
        .header("host", "plain-aware-peer")
        .header("c-id", actor)
        .header("c-cid", channel)
        .header("content-type", "application/octet-stream")
        .body(RequestBody((!body.is_empty()).then(|| body.into())))?)
}

pub struct Response {
    pub status: u16,
    pub body: hyper::body::Incoming,
    pub lease: Lease,
}
pub async fn aware(
    state: &CoreRuntime,
    peer: &plain_rs::db::DPeer,
    method: &str,
    path: &str,
    channel: &str,
    body: Vec<u8>,
) -> Result<Response, Failure> {
    let (socket, mut lease) = socket(state, peer)
        .await
        .map_err(|e| Failure::Unavailable(e.to_string()))?;
    let tls = tokio_rustls::TlsConnector::from(Arc::new(
        plain_rs::services::peers::status_socket::config()
            .map_err(|e| Failure::Fatal(e.to_string()))?,
    ))
    .connect(
        rustls::pki_types::ServerName::try_from("plain-aware-peer")
            .map_err(|e| Failure::Fatal(e.to_string()))?,
        socket,
    )
    .await
    .map_err(|e| Failure::Unavailable(e.to_string()))?;
    let (mut sender, connection) =
        hyper::client::conn::http1::handshake(hyper_util::rt::TokioIo::new(tls))
            .await
            .map_err(|e| Failure::Unavailable(e.to_string()))?;
    lease.jobs.push(tokio::spawn(async move {
        let _ = connection.await;
    }));
    let actor = state
        .prefs
        .get::<String>(plain_rs::prefs::pref_keys::CLIENT_ID)
        .map_err(|e| Failure::Fatal(e.to_string()))?
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Failure::Fatal("Missing peer actor".into()))?;
    let request = aware_request(method, path, &actor, channel, body)
        .map_err(|error| Failure::Fatal(error.to_string()))?;
    let response = sender
        .send_request(request)
        .await
        .map_err(|e| Failure::Fatal(e.to_string()))?;
    Ok(Response {
        status: response.status().as_u16(),
        body: response.into_body(),
        lease,
    })
}
pub async fn aware_send(
    state: &CoreRuntime,
    peer: &plain_rs::db::DPeer,
    channel: &str,
    key: &[u8],
    body: &str,
) -> Result<Value, Failure> {
    let encrypted = plain_rs::crypto::xchacha_encrypt_raw(key, body.as_bytes())
        .ok_or_else(|| Failure::Fatal("Invalid peer key".into()))?;
    let context = plain_rs::transport::http_response::Context::new("/peer_graphql", &encrypted);
    let response = aware(state, peer, "POST", "/peer_graphql", channel, encrypted).await?;
    async {
        let status = response.status;
        let _lease = response.lease;
        let bytes = plain_rs::services::peers::peer_lan::limited_chunks(
            plain_rs::services::peers::peer_lan::incoming_chunks(response.body),
            4 * 1024 * 1024,
        )
        .await?;
        decode_response(status, key, &context, &bytes)
    }
    .await
    .map_err(|e: anyhow::Error| Failure::Fatal(e.to_string()))
}
fn decode_response(
    status: u16,
    key: &[u8],
    context: &plain_rs::transport::http_response::Context,
    bytes: &[u8],
) -> Result<Value> {
    let plain = context
        .decrypt(key, bytes)
        .ok_or_else(|| anyhow::anyhow!("Failed to authenticate peer response"))?;
    if status != 200 {
        return Ok(json!({"data":null,"errors":[{"message":format!("Peer HTTP {status}")}]}));
    }
    let value: Value = serde_json::from_slice(&plain)?;
    ensure!(value.is_object(), "Invalid peer response");
    Ok(value)
}
pub async fn ble(
    state: &CoreRuntime,
    peer: &plain_rs::db::DPeer,
    request: plain_rs::protocols::ble::Request,
) -> Result<Vec<u8>, Failure> {
    state
        .ble_transport
        .exchange(
            state,
            peer,
            request
                .encode()
                .map_err(|e| Failure::Fatal(e.to_string()))?,
        )
        .await
        .map_err(|e| Failure::Fatal(e.to_string()))
}
pub fn ble_actor(state: &CoreRuntime) -> Result<String, Failure> {
    state
        .prefs
        .get::<String>(plain_rs::prefs::pref_keys::CLIENT_ID)
        .map_err(|e| Failure::Fatal(e.to_string()))?
        .filter(|s| !s.is_empty())
        .ok_or_else(|| Failure::Fatal("Missing peer actor".into()))
}
pub async fn ble_send(
    state: &CoreRuntime,
    peer: &plain_rs::db::DPeer,
    channel: &str,
    key: &[u8],
    body: &str,
) -> Result<Value, Failure> {
    let encrypted = plain_rs::crypto::xchacha_encrypt_raw(key, body.as_bytes())
        .ok_or_else(|| Failure::Fatal("Invalid peer key".into()))?;
    let context = plain_rs::transport::http_response::Context::new("/peer_graphql", &encrypted);
    let response = ble(
        state,
        peer,
        plain_rs::protocols::ble::Request::PeerGraphql {
            client_id: ble_actor(state)?,
            channel_id: channel.into(),
            body: encrypted,
        },
    )
    .await?;
    let (status, bytes) = plain_rs::protocols::ble::decode_response(&response)
        .map_err(|e| Failure::Fatal(e.to_string()))?;
    decode_response(status, key, &context, bytes).map_err(|e| Failure::Fatal(e.to_string()))
}

struct RequestBody(Option<bytes::Bytes>);
impl hyper::body::Body for RequestBody {
    type Data = bytes::Bytes;
    type Error = std::convert::Infallible;
    fn poll_frame(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<hyper::body::Frame<Self::Data>, Self::Error>>> {
        std::task::Poll::Ready(
            self.0
                .take()
                .map(|bytes| Ok(hyper::body::Frame::data(bytes))),
        )
    }
    fn is_end_stream(&self) -> bool {
        self.0.is_none()
    }
    fn size_hint(&self) -> hyper::body::SizeHint {
        hyper::body::SizeHint::with_exact(self.0.as_ref().map_or(0, |bytes| bytes.len() as u64))
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/services/peers/peer_sdk.rs"]
mod tests;
