use crate::features::chat::enums::DeviceType;
use crate::features::peers::pairing::protocol::PairingCancel;
use crate::features::peers::pairing::protocol::PairingRequestPayload;
use crate::features::peers::pairing::protocol::{PairingAccept, PairingReject, PairingReply};
use plain_libs::anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverReply {
    pub id: String,
    pub name: String,
    pub port: u16,
    #[serde(default)]
    pub remote_access: Option<bool>,
    pub signature_public_key: String,
    pub device_type: DeviceType,
    pub version: String,
    pub platform: String,
    #[serde(default)]
    pub ips: Vec<String>,
    #[serde(default)]
    pub aware_supported: bool,
    #[serde(default)]
    pub aware_running: bool,
}
#[derive(Deserialize, Serialize)]
#[serde(
    tag = "kind",
    content = "payload",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum Message {
    Discover,
    DiscoverReply(DiscoverReply),
    PairRequest(PairingRequestPayload),
    PairingAccept(PairingAccept),
    PairingReject(PairingReject),
    PairCancel(PairingCancel),
}
impl From<PairingReply> for Message {
    fn from(reply: PairingReply) -> Self {
        match reply { PairingReply::Accept(value) => Self::PairingAccept(value), PairingReply::Reject(value) => Self::PairingReject(value) }
    }
}
impl Message {
    pub fn wire(&self) -> Result<String> {
        let value = plain_libs::serde_json::to_value(self)?;
        let kind = value["kind"]
            .as_str()
            .expect("serialized nearby message kind");
        Ok(format!(
            "{kind}:{}",
            value
                .get("payload")
                .map(plain_libs::serde_json::Value::to_string)
                .unwrap_or_default()
        ))
    }
    pub fn parse(body: &str) -> Result<Self> {
        let (kind, payload) = body
            .split_once(':')
            .ok_or_else(|| plain_libs::anyhow::anyhow!("Missing nearby message prefix"))?;
        let mut value = plain_libs::serde_json::json!({"kind":kind});
        if !payload.is_empty() {
            value["payload"] = plain_libs::serde_json::from_str(payload)?;
        }
        Ok(plain_libs::serde_json::from_value(value)?)
    }
}
pub fn discover_reply(payload: &str, advertised_id: &str) -> Result<DiscoverReply> {
    let reply: DiscoverReply = plain_libs::serde_json::from_str(payload)?;
    ensure!(
        crate::features::client_identity::parse(&reply.id).is_some() && reply.id == advertised_id,
        "BLE discovery identity mismatch"
    );
    Ok(reply)
}
#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/features/discovery/nearby_wire.rs"]
mod tests;
