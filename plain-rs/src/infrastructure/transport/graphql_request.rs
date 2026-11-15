use crate::infrastructure::db::Db;
use plain_libs::serde_json::{Map, Value};
use plain_libs::utils::uuid::Uuid;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    pub request_id: String,
    #[serde(flatten)]
    pub content: Map<String, Value>,
}
impl Envelope {
    pub fn new(payload: &str) -> Result<Self, &'static str> {
        let mut content: Map<String, Value> =
            plain_libs::serde_json::from_str(payload).map_err(|_| "Invalid GraphQL request")?;
        content.remove("requestId");
        Ok(Self {
            request_id: crate::features::record_id::generate(),
            content,
        })
    }
    pub fn parse(payload: &str) -> Result<Self, &'static str> {
        let envelope: Self =
            plain_libs::serde_json::from_str(payload).map_err(|_| "Invalid GraphQL request")?;
        if envelope.content.contains_key("signature") {
            return Err("Peer signatures require peer GraphQL transport");
        }
        Ok(envelope)
    }
    pub fn id(&self) -> Option<RequestId> {
        RequestId::parse(&self.request_id)
    }
    pub fn json(&self) -> Result<String, &'static str> {
        plain_libs::serde_json::to_string(self).map_err(|_| "Invalid GraphQL request")
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SignedEnvelope {
    pub signature: String,
    pub request_id: String,
    pub query: String,
    pub variables: Map<String, Value>,
}
impl SignedEnvelope {
    pub fn signing_bytes(&self, channel_id: &str) -> Vec<u8> {
        plain_libs::serde_json::to_vec(&(
            crate::infrastructure::signature::SignaturePurpose::PeerRequest,
            channel_id,
            &self.request_id,
            &self.query,
            &self.variables,
        ))
        .expect("peer signing tuple is JSON")
    }
    pub(super) fn graphql_json(&self) -> String {
        plain_libs::serde_json::json!({
            "requestId": self.request_id,
            "query": self.query,
            "variables": self.variables,
        })
        .to_string()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RequestId(Uuid);
impl RequestId {
    pub fn parse(value: &str) -> Option<Self> {
        if value.len() != 36 {
            return None;
        }
        let id = Uuid::parse_str(value).ok()?;
        let bytes = id.as_bytes();
        (bytes[6] >> 4 == 7 && bytes[8] & 0xc0 == 0x80).then_some(Self(id))
    }
    pub fn timestamp(self) -> i64 {
        let mut bytes = [0; 8];
        bytes[2..].copy_from_slice(&self.0.as_bytes()[..6]);
        i64::from_be_bytes(bytes)
    }
    pub fn is_fresh(self, now: i64, window_ms: u64) -> bool {
        now >= 0 && now.abs_diff(self.timestamp()) <= window_ms
    }
    pub fn admit_persistent(
        self,
        db: &Db,
        scope: &str,
        client: &str,
        now: i64,
        window_ms: i64,
    ) -> plain_libs::rusqlite::Result<bool> {
        db.admit_request(
            scope,
            client,
            self.timestamp(),
            &self.0.to_string(),
            now,
            window_ms,
        )
    }
}
impl std::fmt::Display for RequestId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, formatter)
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/infrastructure/transport/graphql_request.rs"]
mod tests;
