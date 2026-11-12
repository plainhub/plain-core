use crate::infrastructure::signature::SignaturePurpose;
use super::{
    pairing_attempts::PairingPhase,
    pairing_confirmation::{chat_key, save_peer, verification_code},
    pairing_runtime::local_device,
};
use crate::features::{
    discovery::{
        nearby_http,
        nearby_wire::Message,
        pairing_code,
    },
    peers::pairing::{
        now_ms,
        peer_store::Facts,
        protocol::{PairingRequest, PairingRequestPayload, PairingAccept},
        sessions::RESPONSE_TIMEOUT,
    },
    services::peers::peer_wire::signing_keypair,
};
use crate::runtime::Runtime;
use plain_libs::anyhow::{Result, ensure};
use hmac::{Hmac, KeyInit, Mac};
use plain_libs::{
    crypto::{EcdhSession, ed25519_sign, ed25519_verify},
    utils::base64::{base64_decode, base64_encode},
};
use serde::Serialize;
use std::{collections::HashMap, net::IpAddr, sync::Mutex, time::Instant};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Display {
    pub payload: String,
    pub request_id: String,
    pub verification_code: String,
}

#[derive(Default)]
pub struct Pairings {
    displayed: Mutex<Option<Displayed>>,
    scanned: Mutex<HashMap<String, Scanned>>,
}

struct Displayed {
    started: Instant,
    request: PairingRequest,
    ecdh: EcdhSession,
    completed: Option<String>,
}

struct Scanned {
    started: Instant,
    response: PairingAccept,
    facts: Option<Facts>,
    previous_identity: Option<String>,
}

fn response_input(
    request: &PairingRequest,
    response: &PairingAccept,
    sender_id: &str,
) -> Vec<u8> {
    let mut mac =
        Hmac::<sha2_mac::Sha256>::new_from_slice(request.payload.signature.as_bytes()).unwrap();
    mac.update(
        plain_libs::serde_json::json!(["qr-pair-accept", sender_id, response.signature_data()])
            .to_string()
            .as_bytes(),
    );
    plain_libs::serde_json::json!([SignaturePurpose::QrPairAccept, base64_encode(&mac.finalize().into_bytes())])
        .to_string()
        .into_bytes()
}

pub async fn display(state: &Runtime) -> Result<Display> {
    let id = state.native_client_id().map_err(plain_libs::anyhow::Error::msg)?;
    let mut device = local_device(state).await?;
    device.ips.retain(|ip| {
        ip.parse::<IpAddr>()
            .is_ok_and(|ip| !ip.is_loopback() && !ip.is_unspecified())
    });
    ensure!(
        device.port != 0 && !device.ips.is_empty(),
        "Reachable pairing address and active listener required"
    );
    create_display(state, id, device)
}

pub fn create_display(
    state: &Runtime,
    id: String,
    device: super::pairing::Device,
) -> Result<Display> {
    let ecdh = EcdhSession::generate();
    let key = signing_keypair(&state.prefs)?;
    let mut request = PairingRequest {
        from_id: id,
        from_ip: String::new(),
        to_id: String::new(),
        target_public_key: String::new(),
        payload: PairingRequestPayload {
            from_name: device.name,
            port: device.port,
            device_type: device.device_type,
            ips: device.ips,
            signature_public_key: base64_encode(&key[32..]),
            ecdh_public_key: base64_encode(&ecdh.public_key_bytes),
            timestamp: now_ms(),
            signature: String::new(),
            aware_supported: device.aware_supported,
        },
    };
    request.payload.signature = ed25519_sign(&key, request.signature_data().as_bytes());
    let display = Display {
        payload: pairing_code::format(&request)?,
        request_id: request.payload.request_id(),
        verification_code: verification_code(&request),
    };
    *state.pairing_runtime.qr.displayed.lock().unwrap() = Some(Displayed {
        started: Instant::now(),
        request,
        ecdh,
        completed: None,
    });
    Ok(display)
}

pub async fn respond(
    state: &Runtime,
    payload: pairing_code::Payload,
    generation: &str,
) -> Result<()> {
    let request = payload.into_request();
    let sender = state.native_client_id().map_err(plain_libs::anyhow::Error::msg)?;
    ensure!(request.from_id != sender, "Cannot pair with this device");
    let address = crate::infrastructure::transport::lan::best_host(
        &request.payload.ips,
        &crate::infrastructure::transport::lan::local_interfaces(),
    );
    ensure!(!address.is_empty(), "No reachable pairing address");
    state.pairing_runtime.attempts.set_verification_code(
        state,
        generation,
        &verification_code(&request),
    );
    state
        .pairing_runtime
        .attempts
        .transition(state, generation, PairingPhase::Pairing, None, None);
    let device = local_device(state).await?;
    ensure!(device.port != 0, "Active listener required");
    let response = prepare_response(state, &request, &sender, &address, device)?;
    let message = Message::PairingAccept(response);
    loop {
        if nearby_http::send(&address, request.payload.port, &message, Some(&sender)).await? {
            commit_scanned(state, &request, &address)?;
            state.pairing_runtime.attempts.transition(
                state,
                generation,
                PairingPhase::Succeeded,
                None,
                None,
            );
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}

fn prepare_response(
    state: &Runtime,
    request: &PairingRequest,
    sender: &str,
    address: &str,
    device: super::pairing::Device,
) -> Result<PairingAccept> {
    let _commit = state.pairing_runtime.commit.lock().unwrap();
    let mut scanned = state.pairing_runtime.qr.scanned.lock().unwrap();
    scanned.retain(|_, entry| entry.started.elapsed() < RESPONSE_TIMEOUT);
    let id = request.payload.request_id();
    if let Some(entry) = scanned.get(&id) {
        return Ok(entry.response.clone());
    }
    ensure!(scanned.len() < 128, "Pairing capacity exceeded");
    let key = signing_keypair(&state.prefs)?;
    let ecdh = EcdhSession::generate();
    let shared = ecdh
        .compute_shared_key(&base64_decode(&request.payload.ecdh_public_key))
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Invalid pairing key"))?;
    let mut response = PairingAccept {
        request_id: id.clone(),
        signature_public_key: Some(base64_encode(&key[32..])),
        from_name: Some(device.name),
        port: device.port,
        device_type: device.device_type,
        ips: device.ips,
        ecdh_public_key: base64_encode(&ecdh.public_key_bytes),
        signature: String::new(),
        aware_supported: device.aware_supported,
    };
    response.signature = ed25519_sign(&key, &response_input(request, &response, sender));
    let previous = crate::infrastructure::db::chat_store::peers::get(&state.db, &request.from_id)?;
    let facts = Facts {
        id: request.from_id.clone(),
        name: request.payload.from_name.clone(),
        ips: std::iter::once(address.to_owned())
            .chain(request.payload.ips.clone())
            .collect(),
        port: request.payload.port,
        device_type: request.payload.device_type,
        key: base64_encode(&chat_key(&shared, request, &response)),
        public_key: request.payload.signature_public_key.clone(),
    };
    scanned.insert(
        id,
        Scanned {
            started: Instant::now(),
            response: response.clone(),
            facts: Some(facts),
            previous_identity: previous.map(|peer| peer.public_key),
        },
    );
    Ok(response)
}

fn commit_scanned(state: &Runtime, request: &PairingRequest, address: &str) -> Result<()> {
    let _commit = state.pairing_runtime.commit.lock().unwrap();
    let mut scanned = state.pairing_runtime.qr.scanned.lock().unwrap();
    let entry = scanned
        .get_mut(&request.payload.request_id())
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Pairing QR expired"))?;
    if let Some(facts) = &entry.facts {
        save_peer(
            state,
            facts.clone(),
            entry.previous_identity.as_deref(),
            address,
            None,
        )?;
        entry.facts = None;
    }
    Ok(())
}

pub(super) fn complete(
    state: &Runtime,
    response: PairingAccept,
    address: IpAddr,
    sender: &str,
) -> Result<()> {
    let _commit = state.pairing_runtime.commit.lock().unwrap();
    let mut displayed = state.pairing_runtime.qr.displayed.lock().unwrap();
    let entry = displayed
        .as_mut()
        .filter(|entry| {
            entry.request.payload.request_id() == response.request_id
                && entry.started.elapsed() < RESPONSE_TIMEOUT
        })
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Pairing QR expired"))?;
    let request = &entry.request;
    let name = response.from_name.as_deref().unwrap_or_default();
    let key = response.signature_public_key.as_deref().unwrap_or_default();
    ensure!(
        !sender.is_empty()
            && sender.len() <= 256
            && request.from_id != sender
            && !name.trim().is_empty()
            && response.port != 0
            && ed25519_verify(
                key,
                &response_input(request, &response, sender),
                &response.signature
            ),
        "Invalid pairing QR response"
    );
    if let Some(signature) = &entry.completed {
        ensure!(*signature == response.signature, "Pairing QR already used");
        return Ok(());
    }
    let shared = entry
        .ecdh
        .compute_shared_key(&base64_decode(&response.ecdh_public_key))
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Invalid pairing key"))?;
    let previous = crate::infrastructure::db::chat_store::peers::get(&state.db, sender)?;
    let address = address.to_string();
    save_peer(
        state,
        Facts {
            id: sender.to_owned(),
            name: name.to_owned(),
            ips: std::iter::once(address.clone())
                .chain(response.ips.clone())
                .collect(),
            port: response.port,
            device_type: response.device_type,
            key: base64_encode(&chat_key(&shared, request, &response)),
            public_key: key.to_owned(),
        },
        previous.as_ref().map(|peer| peer.public_key.as_str()),
        &address,
        Some(&response.request_id),
    )?;
    entry.completed = Some(response.signature);
    Ok(())
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/discovery/pairing_qr.rs"]
mod tests;
