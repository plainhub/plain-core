use crate::runtime::Runtime as CoreRuntime;
use anyhow::Result;
use plain_rs::protocols::ble;
use std::time::Duration;
#[derive(Default)]
pub struct Runtime;
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
        state: &CoreRuntime,
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
            "peer": plain_rs::services::peers::peer_address::view(peer),
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

#[cfg(any(test, feature = "test-seams"))]
impl Runtime {
    /// Decodes the request bytes the host was asked to send.
    pub fn request_for_test(&self, params: &serde_json::Value) -> ble::Request {
        let bytes = plain_rs::utils::base64::base64_decode(
            params["request"].as_str().expect("host request bytes"),
        );
        ble::Request::decode(&bytes).unwrap()
    }
    /// Builds the host reply body for a given encoded response.
    pub fn reply_for_test(&self, _params: &serde_json::Value, bytes: Vec<u8>) -> serde_json::Value {
        serde_json::json!({
            "response": plain_rs::utils::base64::base64_encode(&bytes),
        })
    }
}

use plain_rs::protocols::ble::Request;
#[cfg(any(test, feature = "test-seams"))]
pub fn test_peer_wire(runtime: &Runtime, params: &serde_json::Value, key: &[u8]) -> String {
    let Request::PeerGraphql { body, .. } = runtime.request_for_test(params) else {
        panic!("Expected peer request")
    };
    String::from_utf8(plain_rs::crypto::xchacha_decrypt_raw(key, &body).unwrap()).unwrap()
}
#[cfg(any(test, feature = "test-seams"))]
pub fn test_reply(
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
