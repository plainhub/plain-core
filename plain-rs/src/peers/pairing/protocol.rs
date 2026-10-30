use serde::{Deserialize, Serialize};

use crate::peers::pairing::utils::device_type_signature_value;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PairingRequest {
    pub from_id: String,
    pub from_name: String,
    pub port: u16,
    pub device_type: String,
    pub ecdh_public_key: String,
    pub signature_public_key: String,
    pub timestamp: i64,
    #[serde(default)]
    pub ips: Vec<String>,
    #[serde(default)]
    pub signature: String,
    #[serde(default)]
    pub aware_supported: bool,
    // The receiver supplies this return address; it is not signed by the sender.
    #[serde(default)]
    pub from_ip: String,
}

impl PairingRequest {
    pub fn signature_data(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}",
            self.from_id,
            self.from_name,
            self.port,
            device_type_signature_value(&self.device_type),
            self.ecdh_public_key,
            self.signature_public_key,
            self.timestamp,
            self.ips.join(",")
        )
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PairingResponse {
    pub confirmation_proof: String,
    pub request_signature: String,
    pub from_id: String,
    pub to_id: String,
    pub port: u16,
    pub device_type: String,
    pub ecdh_public_key: String,
    pub signature_public_key: String,
    pub accepted: bool,
    pub timestamp: i64,
    #[serde(default)]
    pub ips: Vec<String>,
    #[serde(default)]
    pub signature: String,
    #[serde(default)]
    pub aware_supported: bool,
}

impl PairingResponse {
    pub fn signature_data(&self) -> String {
        format!(
            "plain-pair-response-v2|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            self.request_signature,
            self.from_id,
            self.to_id,
            self.port,
            device_type_signature_value(&self.device_type),
            self.ecdh_public_key,
            self.signature_public_key,
            self.accepted,
            self.timestamp,
            self.ips.join(",")
        )
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
        format!(
            "plain-pair-cancel-v2|{}|{}|{}|{}|{}",
            self.from_id,
            self.to_id,
            self.request_signature,
            self.signature_public_key,
            self.timestamp
        )
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/peers/pairing/protocol.rs"]
mod tests;
