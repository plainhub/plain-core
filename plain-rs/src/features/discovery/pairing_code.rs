use super::nearby_devices::{Device, DiscoveryMethod};
use crate::features::peers::pairing::{
    protocol::{PairingRequest, PairingRequestPayload},
    timestamp_ok,
};
use plain_libs::anyhow::{Result, ensure};
use plain_libs::utils::query::{percent_decode, url_encode};
use serde::{Deserialize, Serialize};

const PREFIX: &str = "plainapp://pair?request=";
const MAX_QR_LEN: usize = 2048;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Payload {
    pub from_id: String,
    #[serde(flatten)]
    pub request: PairingRequestPayload,
}

impl Payload {
    pub fn into_request(self) -> PairingRequest {
        PairingRequest {
            from_id: self.from_id,
            from_ip: String::new(),
            to_id: String::new(),
            target_public_key: String::new(),
            payload: self.request,
        }
    }
}

pub fn format(request: &PairingRequest) -> Result<String> {
    let payload = Payload {
        from_id: request.from_id.clone(),
        request: request.payload.clone(),
    };
    let code = format!("{PREFIX}{}", url_encode(&serde_json::to_string(&payload)?));
    ensure!(code.len() <= MAX_QR_LEN, "Pairing QR exceeds limit");
    Ok(code)
}

pub fn parse(code: &str) -> Result<Payload> {
    ensure!(code.len() <= MAX_QR_LEN, "Pairing QR exceeds limit");
    let encoded = code
        .strip_prefix(PREFIX)
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Invalid pairing QR"))?;
    let payload: Payload = serde_json::from_str(&percent_decode(encoded))?;
    let request = payload.into_request();
    let body = &request.payload;
    ensure!(
        !request.from_id.is_empty()
            && request.from_id.len() <= 256
            && !body.from_name.trim().is_empty()
            && body.port != 0
            && !body.ips.is_empty()
            && body.ips.len() <= 64
            && body
                .ips
                .iter()
                .all(|ip| ip.parse::<std::net::IpAddr>().is_ok())
            && plain_libs::utils::base64::base64_decode(&body.ecdh_public_key).len() == 65
            && timestamp_ok(body.timestamp)
            && plain_libs::crypto::ed25519_verify(
                &body.signature_public_key,
                request.signature_data().as_bytes(),
                &body.signature
            ),
        "Invalid or expired pairing QR"
    );
    Ok(Payload {
        from_id: request.from_id,
        request: request.payload,
    })
}

impl Payload {
    pub fn into_device(self) -> Device {
        Device {
            id: self.from_id,
            name: self.request.from_name,
            ips: self.request.ips,
            port: self.request.port,
            device_type: self.request.device_type,
            version: String::new(),
            platform: String::new(),
            last_seen_at: crate::infrastructure::db::now_iso(),
            remote_access: None,
            signature_public_key: self.request.signature_public_key,
            discovery_methods: vec![DiscoveryMethod::Lan, DiscoveryMethod::Qr],
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/features/discovery/pairing_code.rs"]
mod tests;
