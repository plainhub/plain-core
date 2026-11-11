use crate::features::peers::pairing::now_ms;
use crate::features::peers::pairing::protocol::PairingResponse;
use crate::features::peers::pairing::protocol::{PairingRequest, PairingRequestPayload};
use crate::features::peers::pairing::security;
use crate::features::prefs::Prefs;
use crate::features::services::peers::peer_wire::signing_keypair;
use plain_libs::anyhow::{Result, bail};
use plain_libs::crypto::EcdhSession;
use serde::Deserialize;
use serde_json::{Value, json};
#[derive(Deserialize)]
pub struct Device {
    pub name: String,
    pub port: u16,
    pub device_type: crate::features::chat::enums::DeviceType,
    pub ips: Vec<String>,
    pub aware_supported: bool,
}
fn actor(prefs: &Prefs) -> Result<String> {
    prefs
        .get::<String>(crate::features::prefs::pref_keys::CLIENT_ID)?
        .filter(|s| !s.is_empty())
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Missing pairing identity"))
}
fn request(
    prefs: &Prefs,
    device: Device,
    target: &Target,
 ) -> Result<(PairingRequest, EcdhSession)> {
    let kp = signing_keypair(prefs)?;
    let ecdh = EcdhSession::generate();
    let mut request = PairingRequest {
        to_id: target.device_id.clone(),
        target_public_key: target.device_public_key.clone(),
        from_id: actor(prefs)?,
        from_ip: String::new(),
        payload: PairingRequestPayload {
            from_name: device.name,
            port: device.port,
            device_type: device.device_type,
            ecdh_public_key: plain_libs::utils::base64::base64_encode(&ecdh.public_key_bytes),
            signature_public_key: plain_libs::utils::base64::base64_encode(&kp[32..]),
            timestamp: now_ms(),
            ips: device.ips,
            signature: String::new(),
            aware_supported: device.aware_supported,
        },
    };
    request.payload.signature =
        plain_libs::crypto::ed25519_sign(&kp, request.signature_data().as_bytes());
    if !security::verify_request(&request) {
        bail!("Invalid pairing signing identity");
    }
    Ok((request, ecdh))
}
pub(super) fn response(
    prefs: &Prefs,
    request: PairingRequest,
    accepted: bool,
    device: Device,
) -> Result<Option<(PairingResponse, Option<EcdhSession>)>> {
    if !security::verify_request(&request) {
        return Ok(None);
    }
    let kp = signing_keypair(prefs)?;
    let local_key = plain_libs::utils::base64::base64_encode(&kp[32..]);
    plain_libs::anyhow::ensure!(
        request.to_id == actor(prefs)?
            && request.target_public_key == local_key,
        "Pairing target identity mismatch"
    );
    let ecdh = accepted.then(EcdhSession::generate);
    let mut response = PairingResponse {
        request_id: request.payload.request_id(),
        signature_public_key: None,
        from_name: None,
        port: device.port,
        device_type: device.device_type,
        ecdh_public_key: ecdh
            .as_ref()
            .map(|e| plain_libs::utils::base64::base64_encode(&e.public_key_bytes))
            .unwrap_or_default(),
        accepted,
        ips: device.ips,
        signature: String::new(),
        aware_supported: device.aware_supported,
    };
    response.signature =
        plain_libs::crypto::ed25519_sign(&kp, response.signature_data().as_bytes());
    Ok(Some((response, ecdh)))
}
use crate::features::peers::pairing::protocol::PairingCancel;
use crate::features::peers::pairing::sessions::Sessions;
use crate::features::peers::pairing::sessions::Target;
#[cfg(test)]
pub fn start(prefs: &Prefs, sessions: &Sessions, target: Target, device: Device) -> Result<Value> {
    start_generation(prefs, sessions, target, device, None)
}
pub fn start_generation(
    prefs: &Prefs,
    sessions: &Sessions,
    target: Target,
    device: Device,
    generation: Option<String>,
) -> Result<Value> {
    if target.device_id.is_empty() {
        bail!("Missing pairing target");
    }
    let (request, ecdh) = request(prefs, device, &target)?;
    let ticket = match generation {
        Some(generation) => sessions.start_generation(target, ecdh, generation),
        None => sessions.start(target, ecdh),
    };
    let ticket = sessions
        .bind_request(
            &ticket.target.device_id,
            &ticket.generation,
            &request.payload.signature,
        )
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Pairing replaced"))?;
    Ok(json!({"request":request,"ticket":ticket}))
}
pub fn cancel(
    prefs: &Prefs,
    sessions: &Sessions,
    id: &str,
    generation: Option<&str>,
) -> Result<Value> {
    let from_id = actor(prefs)?;
    let Some(ticket) = sessions.cancel(id, generation) else {
        return Ok(Value::Null);
    };
    let keypair = signing_keypair(prefs)?;
    let mut cancel = PairingCancel {
        from_id,
        to_id: id.into(),
        request_signature: ticket.request_signature.clone(),
        signature_public_key: plain_libs::utils::base64::base64_encode(&keypair[32..]),
        timestamp: now_ms(),
        signature: String::new(),
    };
    cancel.signature =
        plain_libs::crypto::ed25519_sign(&keypair, cancel.signature_data().as_bytes());
    Ok(json!({"ticket":ticket,"cancel":cancel}))
}
#[cfg(test)]
pub fn receive_request(sessions: &Sessions, request: &PairingRequest) -> Option<bool> {
    security::verify_request(request).then(|| sessions.receive(request))
}
pub fn receive_cancel(prefs: &Prefs, sessions: &Sessions, cancel: PairingCancel) -> Result<Value> {
    if !security::verify_cancel(&cancel, &actor(prefs)?) {
        return Ok(Value::Null);
    }
    Ok(serde_json::to_value(sessions.receive_cancel(&cancel))?)
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/discovery/pairing.rs"]
mod tests;
