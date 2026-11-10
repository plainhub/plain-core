use serde::{Deserialize, Serialize};

use crate::features::chat::enums::DeviceType;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PairingRequest {
    pub to_id: String,
    pub target_public_key: String,
    pub from_id: String,
    #[serde(default)]
    pub from_ip: String,
    #[serde(flatten)]
    pub payload: PairingRequestPayload,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PairingRequestPayload {
    pub from_name: String,
    pub port: u16,
    pub device_type: DeviceType,
    pub ecdh_public_key: String,
    pub signature_public_key: String,
    pub timestamp: i64,
    #[serde(default)]
    pub ips: Vec<String>,
    #[serde(default)]
    pub signature: String,
    #[serde(default)]
    pub aware_supported: bool,
}

impl PairingRequest {
    pub fn signature_data(&self) -> String {
        serde_json::json!([
            "pair-request",
            self.from_id,
            self.payload.signature_public_key,
            self.to_id,
            self.target_public_key,
            self.payload.from_name,
            self.payload.port,
            self.payload.device_type.as_str(),
            self.payload.ecdh_public_key,
            self.payload.timestamp,
            self.payload.ips,
            self.payload.aware_supported,
        ])
        .to_string()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PairingResponse {
    pub request_signature: String,
    pub port: u16,
    pub device_type: DeviceType,
    pub ecdh_public_key: String,
    pub accepted: bool,
    #[serde(default)]
    pub ips: Vec<String>,
    #[serde(default)]
    pub signature: String,
    #[serde(default)]
    pub aware_supported: bool,
}

impl PairingResponse {
    pub fn signature_data(&self) -> String {
        serde_json::json!([
            "pair-response",
            self.request_signature,
            self.port,
            self.device_type.as_str(),
            self.ecdh_public_key,
            self.accepted,
            self.ips,
            self.aware_supported,
        ])
        .to_string()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PairingCancel {
    pub from_id: String,
    pub to_id: String,
    pub request_signature: String,
    pub signature_public_key: String,
    pub timestamp: i64,
    pub signature: String,
}
impl PairingCancel {
    pub fn signature_data(&self) -> String {
        serde_json::json!([
            "pair-cancel",
            self.from_id,
            self.to_id,
            self.request_signature,
            self.signature_public_key,
            self.timestamp,
        ])
        .to_string()
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/peers/pairing/protocol.rs"]
mod tests;
