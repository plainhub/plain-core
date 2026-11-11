use crate::features::peers::pairing::{
    peer_store::{self, Facts},
    protocol::{PairingRequest, PairingResponse},
    sessions::{RESPONSE_TIMEOUT, Target, Ticket},
};
use crate::features::services::discovery::{
    pairing, pairing_attempts::PairingPhase, pairing_runtime,
};
use crate::runtime::Runtime as CoreRuntime;
use plain_libs::anyhow::{Result, ensure};
use hmac::{Hmac, KeyInit, Mac};
use plain_libs::utils::base64::{base64_decode, base64_encode};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct Confirmations(Mutex<HashMap<String, Entry>>);
struct Entry {
    started: Instant,
    request: PairingRequest,
    previous_identity: Option<String>,
    phase: Phase,
}
enum Phase {
    Client {
        ticket: Ticket,
        pending: Option<(String, Facts)>,
        completed: bool,
    },
    Server {
        response: Option<PairingResponse>,
    },
}
fn derive(input: &[u8], salt: &[u8], info: &[u8]) -> [u8; 32] {
    let mut extract = Hmac::<sha2_mac::Sha256>::new_from_slice(salt).unwrap();
    extract.update(input);
    let mut expand =
        Hmac::<sha2_mac::Sha256>::new_from_slice(&extract.finalize().into_bytes()).unwrap();
    expand.update(info);
    expand.update(&[1]);
    expand.finalize().into_bytes().into()
}
pub(super) fn chat_key(shared: &[u8; 32], request: &PairingRequest, response: &PairingResponse) -> [u8; 32] {
    let context = Sha256::digest(
        plain_libs::serde_json::json!([
            "pairing",
            request.signature_data(),
            request.payload.signature,
            response.signature_data(),
            response.signature
        ])
        .to_string()
        .as_bytes(),
    );
    derive(shared, &context, b"api")
}

pub(super) fn verification_code(request: &PairingRequest) -> String {
    let digest = Sha256::digest(request.payload.signature.as_bytes());
    let digits = u64::from_be_bytes(digest[..8].try_into().unwrap()) % 100_000_000;
    format!("{digits:08}")
}

pub(super) fn save_peer(
    state: &CoreRuntime,
    facts: Facts,
    previous_identity: Option<&str>,
    ip: &str,
    request_id: Option<&str>,
) -> Result<()> {
    let peer = peer_store::save_confirmed(&state.db, facts, previous_identity, true)?;
    pairing_runtime::success(
        state,
        &plain_libs::serde_json::to_value(crate::features::services::peers::peer_address::view(&peer))?,
        ip,
        request_id,
    );
    Ok(())
}

impl Confirmations {
    fn with<T>(&self, f: impl FnOnce(&mut HashMap<String, Entry>) -> Result<T>) -> Result<T> {
        let mut entries = self.0.lock().unwrap();
        entries.retain(|_, e| e.started.elapsed() < Duration::from_secs(610));
        f(&mut entries)
    }
    pub fn begin(
        &self,
        state: &CoreRuntime,
        request: &PairingRequest,
        ticket: &Ticket,
    ) -> Result<()> {
        let previous_identity =
            crate::infrastructure::db::chat_store::peers::get(&state.db, &ticket.target.device_id)?
                .map(|peer| peer.public_key);
        self.with(|entries| {
            ensure!(entries.len() < 128, "Pairing capacity exceeded");
            entries.insert(
                request.payload.request_id(),
                Entry {
                    started: Instant::now(),
                    request: request.clone(),
                    previous_identity,
                    phase: Phase::Client {
                        ticket: ticket.clone(),
                        pending: None,
                        completed: false,
                    },
                },
            );
            state.pairing_runtime.attempts.set_verification_code(
                state,
                &ticket.generation,
                &verification_code(request),
            );
            Ok(())
        })
    }
    pub fn register(&self, state: &CoreRuntime, request: &PairingRequest) -> Result<bool> {
        let previous_identity =
            crate::infrastructure::db::chat_store::peers::get(&state.db, &request.from_id)?
                .map(|p| p.public_key);
        self.with(|entries| {
            if entries.contains_key(&request.payload.request_id()) {
                return Ok(false);
            }
            ensure!(entries.len() < 128, "Pairing capacity exceeded");
            entries.insert(
                request.payload.request_id(),
                Entry {
                    started: Instant::now(),
                    request: request.clone(),
                    previous_identity,
                    phase: Phase::Server { response: None },
                },
            );
            Ok(true)
        })
    }
    pub fn awaiting_response(&self, state: &CoreRuntime, ticket: &Ticket) -> bool {
        if state
            .pairing
            .remaining_ms(&ticket.target.device_id, &ticket.generation)
            .is_none()
        {
            return false;
        }
        self.0
            .lock()
            .unwrap()
            .get(&crate::features::peers::pairing::protocol::request_id(&ticket.request_signature))
            .is_some_and(|e| {
                matches!(
                    e.phase,
                    Phase::Client {
                        completed: false,
                        ..
                    }
                )
            })
    }

    pub fn cached_response(&self, request: &PairingRequest) -> Result<Option<PairingResponse>> {
        self.with(|entries| {
            Ok(entries.get(&request.payload.request_id())
                .filter(|e| e.request.signature_data() == request.signature_data())
                .and_then(|e| match &e.phase {
                    Phase::Server { response, .. } => response.clone(),
                    _ => None,
                }))
        })
    }
    pub fn respond(
        &self,
        state: &CoreRuntime,
        request: &PairingRequest,
        accepted: bool,
        device: pairing::Device,
    ) -> Result<(PairingResponse, Target)> {
        let _commit = state.pairing_runtime.commit.lock().unwrap();
        self.with(|entries| {
            let e = entries
                .get_mut(&request.payload.request_id())
                .ok_or_else(|| plain_libs::anyhow::anyhow!("Pairing expired"))?;
            ensure!(e.started.elapsed() < RESPONSE_TIMEOUT, "Pairing expired");
            ensure!(
                e.request.signature_data() == request.signature_data(),
                "Pairing request changed"
            );
            let Phase::Server { response } = &mut e.phase else {
                plain_libs::anyhow::bail!("Unexpected pairing role")
            };
            if let Some(reply) = response {
                return Ok((reply.clone(), Target::from(&e.request)));
            }
            let target = state
                .pairing
                .incoming_target(&request.from_id, &request.payload.signature)
                .ok_or_else(|| plain_libs::anyhow::anyhow!("Pairing request expired or no longer available"))?;
            let (reply, ecdh) =
                pairing::response(&state.prefs, e.request.clone(), accepted, device)?
                    .ok_or_else(|| plain_libs::anyhow::anyhow!("Invalid pairing request"))?;
            if let Some(ecdh) = ecdh {
                let shared = ecdh
                    .compute_shared_key(&base64_decode(&request.payload.ecdh_public_key))
                    .ok_or_else(|| plain_libs::anyhow::anyhow!("Invalid pairing key"))?;
                let mut ips = vec![target.device_ip.clone()];
                ips.extend(request.payload.ips.clone());
                save_peer(
                    state,
                    Facts {
                        id: request.from_id.clone(),
                        name: request.payload.from_name.clone(),
                        ips,
                        port: request.payload.port,
                        device_type: request.payload.device_type,
                        key: base64_encode(&chat_key(&shared, &e.request, &reply)),
                        public_key: request.payload.signature_public_key.clone(),
                    },
                    e.previous_identity.as_deref(),
                    &target.device_ip,
                    None,
                )?;
            }
            *response = Some(reply.clone());
            state
                .pairing
                .take_incoming(&request.from_id, &request.payload.signature);
            Ok((reply, target))
        })
    }

    pub fn response(
        &self,
        state: &CoreRuntime,
        response: &PairingResponse,
        sender_ip: &str,
    ) -> Result<()> {
        let _commit = state.pairing_runtime.commit.lock().unwrap();
        self.with(|entries| {
            let e = entries
                .get_mut(&response.request_id)
                .ok_or_else(|| plain_libs::anyhow::anyhow!("Unknown pairing response"))?;
            let Phase::Client {
                ticket,
                pending,
                completed,
            } = &mut e.phase
            else {
                plain_libs::anyhow::bail!("Unexpected pairing role")
            };
            ensure!(
                response.signature_public_key.is_none() && response.from_name.is_none(),
                "Unexpected pairing identity"
            );
            let public_key = &ticket.target.device_public_key;
            ensure!(
                crate::features::peers::pairing::security::verify_response(response, public_key),
                "Pairing identity mismatch"
            );
            if *completed {
                return Ok(());
            }
            ensure!(
                e.started.elapsed() < RESPONSE_TIMEOUT
                    && state
                        .pairing
                        .remaining_ms(&ticket.target.device_id, &ticket.generation)
                        .is_some(),
                "Pairing expired or cancelled"
            );
            if !response.accepted {
                ensure!(pending.is_none(), "Pairing response changed");
                state
                    .pairing
                    .cancel(&ticket.target.device_id, Some(&ticket.generation));
                *completed = true;
                pairing_runtime::failed(
                    state,
                    &ticket.target,
                    "Pairing rejected",
                    Some(&ticket.generation),
                );
                return Ok(());
            }
            if let Some((signature, _)) = pending {
                ensure!(*signature == response.signature, "Pairing response changed");
            } else {
                let shared = state
                    .pairing
                    .shared_key(
                        &ticket.target.device_id,
                        &ticket.request_signature,
                        &base64_decode(&response.ecdh_public_key),
                    )
                    .ok_or_else(|| plain_libs::anyhow::anyhow!("Invalid pairing key"))?;
                let mut ips = vec![sender_ip.to_owned()];
                ips.extend(response.ips.clone());
                *pending = Some((
                    response.signature.clone(),
                    Facts {
                        id: ticket.target.device_id.clone(),
                        name: ticket.target.device_name.clone(),
                        ips,
                        port: response.port,
                        device_type: response.device_type,
                        key: base64_encode(&chat_key(&shared, &e.request, response)),
                        public_key: public_key.to_owned(),
                    },
                ));
            }
            Self::commit_client(state, e)
        })
    }

    fn commit_client(state: &CoreRuntime, e: &mut Entry) -> Result<()> {
        let Phase::Client {
            ticket,
            pending,
            completed,
        } = &mut e.phase
        else {
            plain_libs::anyhow::bail!("Unexpected pairing role")
        };
        if *completed {
            return Ok(());
        }
        ensure!(
            e.started.elapsed() < RESPONSE_TIMEOUT
                && state
                    .pairing
                    .remaining_ms(&ticket.target.device_id, &ticket.generation)
                    .is_some(),
            "Pairing expired or cancelled"
        );
        let (_, facts) = pending
            .as_ref()
            .ok_or_else(|| plain_libs::anyhow::anyhow!("Pairing response required"))?;
        save_peer(
            state,
            facts.clone(),
            e.previous_identity.as_deref(),
            &ticket.target.device_ip,
            None,
        )?;
        *pending = None;
        *completed = true;
        state.pairing.take_response(
            &ticket.target.device_id,
            Some(&ticket.generation),
            &ticket.request_signature,
        );
        state.pairing_runtime.attempts.transition(
            state,
            &ticket.generation,
            PairingPhase::Succeeded,
            None,
            None,
        );
        Ok(())
    }
}

pub async fn confirm(state: &CoreRuntime, generation: &str) -> Result<()> {
    state.pairing_runtime.attempts.trust(generation)
}

pub fn publish(state: &CoreRuntime, request: PairingRequest) -> Result<()> {
    let changed = crate::infrastructure::db::chat_store::peers::get(&state.db, &request.from_id)?
        .is_some_and(|p| p.public_key != request.payload.signature_public_key);
    let mut value = plain_libs::serde_json::to_value(&request)?;
    value["identityChanged"] = plain_libs::serde_json::json!(changed);
    value["verificationCode"] = plain_libs::serde_json::json!(verification_code(&request));
    pairing_runtime::event(
        state,
        crate::features::chat::events::WS_PAIRING_REQUEST_RECEIVED,
        value,
    );
    Ok(())
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/discovery/pairing_confirmation.rs"]
mod tests;
