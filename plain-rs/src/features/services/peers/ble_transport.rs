use crate::infrastructure::protocols::ble;
use crate::runtime::Runtime as CoreRuntime;
use plain_libs::anyhow::Result;
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
        peer: &crate::infrastructure::db::DPeer,
        bytes: Vec<u8>,
    ) -> Result<Vec<u8>> {
        let generation = state
            .host
            .generation()
            .ok_or_else(|| plain_libs::anyhow::anyhow!("BLE host unavailable"))?;
        let request = plain_libs::serde_json::json!({
            "request": plain_libs::utils::base64::base64_encode(&bytes),
            "clientId": peer.id,
            "peer": crate::features::services::peers::peer_address::view(peer),
        });
        let mut stop = state.stop.clone();
        let wait = state.host.call_wait("peerTransportBleExchange", request);
        tokio::pin!(wait);
        let reply = tokio::select! {
            result = tokio::time::timeout(Duration::from_secs(120), wait) => result.map_err(|_| plain_libs::anyhow::anyhow!("BLE exchange timed out"))?,
            _ = stop.changed() => return Err(plain_libs::anyhow::anyhow!("BLE core stopped")),
        }.map_err(plain_libs::anyhow::Error::msg)?;
        if !state.host.is_current(generation) {
            plain_libs::anyhow::bail!("BLE host generation changed");
        }
        let encoded = reply
            .get("response")
            .and_then(plain_libs::serde_json::Value::as_str)
            .ok_or_else(|| plain_libs::anyhow::anyhow!("BLE exchange returned no response"))?;
        let bytes = plain_libs::utils::base64::base64_decode(encoded);
        ble::decode_response(&bytes)?;
        Ok(bytes)
    }
}

#[cfg(any(test, feature = "test-seams"))]
impl Runtime {
    /// Decodes the request bytes the host was asked to send.
    pub fn request_for_test(&self, params: &plain_libs::serde_json::Value) -> ble::Request {
        let bytes = plain_libs::utils::base64::base64_decode(
            params["request"].as_str().expect("host request bytes"),
        );
        ble::Request::decode(&bytes).unwrap()
    }
    /// Builds the host reply body for a given encoded response.
    pub fn reply_for_test(&self, _params: &plain_libs::serde_json::Value, bytes: Vec<u8>) -> plain_libs::serde_json::Value {
        plain_libs::serde_json::json!({
            "response": plain_libs::utils::base64::base64_encode(&bytes),
        })
    }
}

#[cfg(any(test, feature = "test-seams"))]
use crate::infrastructure::protocols::ble::Request;
#[cfg(any(test, feature = "test-seams"))]
pub fn test_peer_wire(runtime: &Runtime, params: &plain_libs::serde_json::Value, key: &[u8]) -> String {
    let Request::PeerGraphql { body, .. } = runtime.request_for_test(params) else {
        panic!("Expected peer request")
    };
    String::from_utf8(
        crate::infrastructure::transport::http_request::decrypt_raw(
            key,
            "POST",
            "/peer_graphql",
            &body,
        )
        .unwrap(),
    )
    .unwrap()
}
#[cfg(any(test, feature = "test-seams"))]
pub fn test_reply(
    runtime: &Runtime,
    params: &plain_libs::serde_json::Value,
    value: plain_libs::serde_json::Value,
    key: &[u8],
) -> plain_libs::serde_json::Value {
    let Request::PeerGraphql { body: request, .. } = runtime.request_for_test(params) else {
        panic!("Expected peer request")
    };
    let body = crate::infrastructure::transport::http_response::Context::new(&request)
        .encrypt(key, value.to_string().as_bytes())
        .unwrap();
    runtime.reply_for_test(params, ble::response(200, &body).unwrap())
}
