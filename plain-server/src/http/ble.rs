use crate::assembly::server::ServerState;
use anyhow::{Result, ensure};
use axum::body::Body;
use plain_rs::protocols::ble;
use plain_rs::protocols::ble::Request;
use std::time::Duration;

mod nearby;

#[derive(Default)]
pub(crate) struct Runtime;
impl Runtime {
    /// Runs one outgoing BLE exchange against a peer.
    ///
    /// The whole GATT exchange happens inside a single host call: the App opens
    /// the link, writes the request frames, reads the response, and returns the
    /// assembled response bytes. There is no same-process websocket hop — the
    /// per-frame codec runs over the fast JNI FFI inside the App, so the only
    /// cross-language cost is this one call per *message*, not one per frame.
    pub async fn exchange(
        &self,
        state: &ServerState,
        peer: &plain_rs::db::DPeer,
        bytes: Vec<u8>,
    ) -> Result<Vec<u8>> {
        let generation = state
            .host
            .generation()
            .ok_or_else(|| anyhow::anyhow!("BLE host unavailable"))?;
        let request = serde_json::json!({
            "request": plain_rs::utils::base64::base64_encode(&bytes),
            "shortId": plain_rs::discovery::nearby_wire::short_id(&peer.id),
            "peer": crate::services::peers::peer_address::view(peer),
        });
        let mut stop = state.stop.clone();
        let wait = state.host.call_wait("peerTransportBleExchange", request);
        tokio::pin!(wait);
        let reply = tokio::select! {
            result = tokio::time::timeout(Duration::from_secs(120), wait) => result.map_err(|_| anyhow::anyhow!("BLE exchange timed out"))?,
            _ = stop.changed() => return Err(anyhow::anyhow!("BLE core stopped")),
        }.map_err(anyhow::Error::msg)?;
        if !state.host.is_current(generation) {
            anyhow::bail!("BLE host generation changed");
        }
        let encoded = reply
            .get("response")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("BLE exchange returned no response"))?;
        let bytes = plain_rs::utils::base64::base64_decode(encoded);
        ble::decode_response(&bytes)?;
        Ok(bytes)
    }
}

fn matches_characteristic(value: &str) -> bool {
    value.eq_ignore_ascii_case(ble::HTTP_CHAR_UUID)
        || value.eq_ignore_ascii_case(ble::NEARBY_CHAR_UUID)
}

pub(crate) async fn dispatch_incoming(
    state: ServerState,
    remote_host: String,
    characteristic_uuid: String,
    bytes: &[u8],
) -> Result<Vec<u8>> {
    ensure!(
        !remote_host.is_empty() && matches_characteristic(&characteristic_uuid),
        "Invalid BLE connection facts"
    );
    ensure!(
        remote_host.len() + characteristic_uuid.len() <= 4096,
        "BLE facts exceed limit"
    );
    ble::parts(bytes)?;
    if characteristic_uuid.eq_ignore_ascii_case(ble::NEARBY_CHAR_UUID) {
        nearby::dispatch(&state, remote_host, bytes).await
    } else {
        dispatch(state, bytes).await
    }
}

pub(crate) async fn dispatch(
    state: ServerState,
    data: &[u8],
) -> Result<Vec<u8>> {
    let request = Request::decode(data)?;
    let (mut builder, body) = match request {
        Request::PeerGraphql {
            client_id,
            channel_id,
            body,
        } => (
            axum::http::Request::builder()
                .method("POST")
                .uri("/peer_graphql")
                .header("c-id", client_id)
                .header("c-cid", channel_id),
            body,
        ),
        Request::FileChunk {
            client_id,
            file_id,
            offset,
            length,
        } => {
            let mut url = reqwest::Url::parse("http://localhost/fs")?;
            url.query_pairs_mut()
                .append_pair("id", &file_id)
                .append_pair("offset", &offset.to_string())
                .append_pair("length", &length.to_string());
            (
                axum::http::Request::builder()
                    .method("GET")
                    .uri(format!("/fs?{}", url.query().unwrap()))
                    .header("c-id", client_id)
                    .header("c-cid", ""),
                Vec::new(),
            )
        }
    };
    builder = builder.header("content-type", "application/octet-stream");
    let mut req = builder.body(Body::from(body))?;
    req.extensions_mut()
        .insert(axum::extract::ConnectInfo(std::net::SocketAddr::from((
            [127, 0, 0, 1],
            0,
        ))));
    #[cfg(feature = "http_transport")]
    {
        req.extensions_mut()
            .insert(crate::http::ConnectionScheme("https"));
    }
    use tower::ServiceExt;
    let response = crate::http::router::transfer_routes(state)
        .oneshot(req)
        .await?;
    let status = response.status().as_u16();
    let body = axum::body::to_bytes(response.into_body(), ble::MAX_BODY).await?;
    ble::response(status, &body)
}

#[cfg(test)]
impl Runtime {
    /// Decodes the request bytes the host was asked to send.
    pub(crate) fn request_for_test(&self, params: &serde_json::Value) -> Request {
        let bytes = plain_rs::utils::base64::base64_decode(
            params["request"].as_str().expect("host request bytes"),
        );
        Request::decode(&bytes).unwrap()
    }
    /// Builds the host reply body for a given encoded response.
    pub(crate) fn reply_for_test(
        &self,
        _params: &serde_json::Value,
        bytes: Vec<u8>,
    ) -> serde_json::Value {
        serde_json::json!({
            "response": plain_rs::utils::base64::base64_encode(&bytes),
        })
    }
}
#[cfg(test)]
pub(crate) fn test_peer_wire(runtime: &Runtime, params: &serde_json::Value, key: &[u8]) -> String {
    let Request::PeerGraphql { body, .. } = runtime.request_for_test(params) else {
        panic!("Expected peer request")
    };
    String::from_utf8(plain_rs::crypto::xchacha_decrypt_raw(key, &body).unwrap()).unwrap()
}
#[cfg(test)]
pub(crate) fn test_reply(
    runtime: &Runtime,
    params: &serde_json::Value,
    value: serde_json::Value,
    key: &[u8],
) -> serde_json::Value {
    let Request::PeerGraphql { body: request, .. } = runtime.request_for_test(params) else {
        panic!("Expected peer request")
    };
    let body = plain_rs::transport::http_response::Context::new("/peer_graphql", &request)
        .encrypt(key, value.to_string().as_bytes())
        .unwrap();
    runtime.reply_for_test(params, ble::response(200, &body).unwrap())
}

#[cfg(test)]
#[path = "../../tests/unit/http/ble.rs"]
mod tests;
