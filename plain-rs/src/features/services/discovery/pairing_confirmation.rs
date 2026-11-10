use crate::features::discovery::nearby_wire::Message;
use crate::features::peers::pairing::{
    confirmation::Verification,
    peer_store::{self, Facts},
    protocol::{PairingRequest, PairingResponse},
    sessions::{RESPONSE_TIMEOUT, Target, Ticket},
};
use crate::features::services::discovery::{
    pairing, pairing_attempts::PairingPhase, pairing_runtime,
};
use crate::runtime::Runtime as CoreRuntime;
use anyhow::{Result, ensure};
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
        channel: Option<Channel>,
        facts: Option<Facts>,
        finish: Option<String>,
        completed: bool,
        delivery_started: bool,
    },
    Server {
        target: Target,
        channel: Option<Channel>,
        facts: Option<Facts>,
        response: Option<PairingResponse>,
        ack: Option<String>,
    },
}
struct Channel {
    key: [u8; 32],
    finish_key: [u8; 32],
    ack_key: [u8; 32],
    context: Vec<u8>,
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
impl Channel {
    fn new(shared: &[u8; 32], request: &PairingRequest, response: &PairingResponse) -> Self {
        let context = Sha256::digest(
            serde_json::json!([
                "pairing",
                request.signature_data(),
                request.payload.signature,
                response.signature_data(),
                response.signature
            ])
            .to_string()
            .as_bytes(),
        )
        .to_vec();
        Self {
            key: derive(shared, &context, b"api"),
            finish_key: derive(shared, &context, b"pair-finish"),
            ack_key: derive(shared, &context, b"pair-ack"),
            context,
        }
    }
    fn proof(&self, ack: bool) -> Result<String> {
        Ok(base64_encode(
            &plain_libs::crypto::xchacha_encrypt_raw(
                if ack { &self.ack_key } else { &self.finish_key },
                &self.context,
            )
            .ok_or_else(|| anyhow::anyhow!("Pairing encryption failed"))?,
        ))
    }
    fn verify(&self, proof: &str, ack: bool) -> Result<()> {
        let plain = plain_libs::crypto::xchacha_decrypt_raw(
            if ack { &self.ack_key } else { &self.finish_key },
            &base64_decode(proof),
        )
        .ok_or_else(|| anyhow::anyhow!("Invalid pairing confirmation"))?;
        ensure!(plain == self.context, "Pairing transcript mismatch");
        Ok(())
    }
}
pub fn identity_digest(request: &PairingRequest) -> Result<String> {
    let a = base64_decode(&request.payload.signature_public_key);
    let b = base64_decode(&request.target_public_key);
    ensure!(a.len() == 32 && b.len() == 32, "Invalid pairing identity");
    let mut hash = Sha256::new();
    hash.update(b"pairing-identity\0initiator\0");
    hash.update(a);
    hash.update(b"\0responder\0");
    hash.update(b);
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}
pub enum Action {
    Notify {
        request: PairingRequest,
        target: Target,
        message: Message,
    },
    Send {
        ticket: Ticket,
        message: Message,
    },
    Done,
}
impl Confirmations {
    fn with<T>(&self, f: impl FnOnce(&mut HashMap<String, Entry>) -> Result<T>) -> Result<T> {
        let mut entries = self.0.lock().unwrap();
        entries.retain(|_, e| e.started.elapsed() < Duration::from_secs(610));
        f(&mut entries)
    }
    pub fn begin(&self, request: &PairingRequest, ticket: &Ticket) -> Result<()> {
        self.with(|entries| {
            ensure!(entries.len() < 128, "Pairing capacity exceeded");
            entries.insert(
                request.payload.signature.clone(),
                Entry {
                    started: Instant::now(),
                    request: request.clone(),
                    previous_identity: None,
                    phase: Phase::Client {
                        ticket: ticket.clone(),
                        channel: None,
                        facts: None,
                        finish: None,
                        completed: false,
                        delivery_started: false,
                    },
                },
            );
            Ok(())
        })
    }
    pub fn register(
        &self,
        state: &CoreRuntime,
        request: &PairingRequest,
        target: Target,
    ) -> Result<bool> {
        let key = crate::features::services::peers::peer_wire::signing_keypair(&state.prefs)?;
        ensure!(
            request.to_id == state.native_client_id().map_err(anyhow::Error::msg)?
                && request.target_public_key == base64_encode(&key[32..]),
            "Pairing target identity mismatch"
        );
        let previous_identity =
            crate::infrastructure::db::chat_store::peers::get(&state.db, &request.from_id)?
                .map(|p| p.public_key);
        self.with(|entries| {
            if entries.contains_key(&request.payload.signature) {
                return Ok(false);
            }
            ensure!(entries.len() < 128, "Pairing capacity exceeded");
            ensure!(
                !entries
                    .values()
                    .any(|e| e.request.from_id == request.from_id
                        && e.started.elapsed() < RESPONSE_TIMEOUT
                        && !matches!(
                            &e.phase,
                            Phase::Server { ack: Some(_), .. }
                                | Phase::Client {
                                    completed: true,
                                    ..
                                }
                        )),
                "Pairing already pending"
            );
            entries.insert(
                request.payload.signature.clone(),
                Entry {
                    started: Instant::now(),
                    request: request.clone(),
                    previous_identity,
                    phase: Phase::Server {
                        target,
                        channel: None,
                        facts: None,
                        response: None,
                        ack: None,
                    },
                },
            );
            Ok(true)
        })
    }
    pub fn cached_response(&self, signature: &str) -> Result<Option<PairingResponse>> {
        self.with(|entries| {
            Ok(entries.get(signature).and_then(|e| match &e.phase {
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
    ) -> Result<PairingResponse> {
        self.with(|entries| {
            let e = entries
                .get_mut(&request.payload.signature)
                .ok_or_else(|| anyhow::anyhow!("Pairing expired"))?;
            ensure!(e.started.elapsed() < RESPONSE_TIMEOUT, "Pairing expired");
            ensure!(
                e.request.signature_data() == request.signature_data(),
                "Pairing request changed"
            );
            let Phase::Server {
                target,
                channel,
                facts,
                response,
                ..
            } = &mut e.phase
            else {
                anyhow::bail!("Unexpected pairing role")
            };
            if let Some(response) = response {
                return Ok(response.clone());
            }
            let (reply, ecdh) =
                pairing::response(&state.prefs, e.request.clone(), accepted, device)?
                    .ok_or_else(|| anyhow::anyhow!("Invalid pairing request"))?;
            if let Some(ecdh) = ecdh {
                let shared = ecdh
                    .compute_shared_key(&base64_decode(&request.payload.ecdh_public_key))
                    .ok_or_else(|| anyhow::anyhow!("Invalid pairing key"))?;
                let c = Channel::new(&shared, request, &reply);
                let mut ips = vec![target.device_ip.clone()];
                ips.extend(request.payload.ips.clone());
                *facts = Some(Facts {
                    id: request.from_id.clone(),
                    name: request.payload.from_name.clone(),
                    ips,
                    port: request.payload.port,
                    device_type: request.payload.device_type,
                    key: base64_encode(&c.key),
                    public_key: request.payload.signature_public_key.clone(),
                });
                *channel = Some(c);
            }
            *response = Some(reply.clone());
            Ok(reply)
        })
    }
    pub fn response(
        &self,
        state: &CoreRuntime,
        response: &PairingResponse,
        sender_ip: &str,
    ) -> Result<Option<Action>> {
        self.with(|entries| {
            let e = entries
                .get_mut(&response.request_signature)
                .ok_or_else(|| anyhow::anyhow!("Unknown pairing response"))?;
            ensure!(e.started.elapsed() < RESPONSE_TIMEOUT, "Pairing expired");
            let Phase::Client {
                ticket,
                channel,
                facts,
                finish,
                ..
            } = &mut e.phase
            else {
                anyhow::bail!("Unexpected pairing role")
            };
            ensure!(
                crate::features::peers::pairing::security::verify_response(
                    response,
                    &ticket.target.device_public_key,
                ),
                "Pairing identity mismatch"
            );
            if !response.accepted {
                state
                    .pairing
                    .cancel(&ticket.target.device_id, Some(&ticket.generation));
                pairing_runtime::failed(
                    state,
                    &ticket.target,
                    "Pairing rejected",
                    Some(&ticket.generation),
                );
                return Ok(None);
            }
            if let Some(proof) = finish {
                return Ok(Some(Action::Send {
                    ticket: ticket.clone(),
                    message: Message::PairVerification(Verification::Finish {
                        request_signature: response.request_signature.clone(),
                        proof: proof.clone(),
                    }),
                }));
            }
            if channel.is_some() {
                return Ok(None);
            }
            if channel.is_none() {
                let shared = state
                    .pairing
                    .shared_key(
                        &ticket.target.device_id,
                        &response.request_signature,
                        &base64_decode(&response.ecdh_public_key),
                    )
                    .ok_or_else(|| anyhow::anyhow!("Pairing expired"))?;
                let c = Channel::new(&shared, &e.request, response);
                let mut ips = vec![sender_ip.to_owned()];
                ips.extend(response.ips.clone());
                *facts = Some(Facts {
                    id: ticket.target.device_id.clone(),
                    name: ticket.target.device_name.clone(),
                    ips,
                    port: response.port,
                    device_type: response.device_type,
                    key: base64_encode(&c.key),
                    public_key: ticket.target.device_public_key.clone(),
                });
                *channel = Some(c);
            }
            let saved = crate::infrastructure::db::chat_store::peers::get(
                &state.db,
                &ticket.target.device_id,
            )?;
            let pinned = saved
                .as_ref()
                .is_some_and(|p| p.is_paired() && p.public_key == ticket.target.device_public_key);
            e.previous_identity = saved.map(|p| p.public_key);
            if pinned {
                return Self::finish(e).map(Some);
            }
            state.pairing_runtime.attempts.set_identity_changed(
                state,
                &ticket.generation,
                e.previous_identity.is_some(),
            );
            state.pairing_runtime.attempts.set_verification_code(
                state,
                &ticket.generation,
                &identity_digest(&e.request)?,
            );
            state.pairing_runtime.attempts.transition(
                state,
                &ticket.generation,
                PairingPhase::TrustRequired,
                None,
                None,
            );
            Ok(None)
        })
    }
    fn finish(e: &mut Entry) -> Result<Action> {
        ensure!(e.started.elapsed() < RESPONSE_TIMEOUT, "Pairing expired");
        let Phase::Client {
            ticket,
            channel,
            finish,
            ..
        } = &mut e.phase
        else {
            anyhow::bail!("Unexpected pairing role")
        };
        let c = channel
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Pairing response required"))?;
        let proof = match finish {
            Some(p) => p.clone(),
            None => {
                let p = c.proof(false)?;
                *finish = Some(p.clone());
                p
            }
        };
        Ok(Action::Send {
            ticket: ticket.clone(),
            message: Message::PairVerification(Verification::Finish {
                request_signature: e.request.payload.signature.clone(),
                proof,
            }),
        })
    }
    pub fn confirm(&self, state: &CoreRuntime, generation: &str) -> Result<Action> {
        self.with(|entries| {
            let e=entries.values_mut().find(|e|matches!(&e.phase,Phase::Client{ticket,..} if ticket.generation==generation)).ok_or_else(||anyhow::anyhow!("Pairing expired"))?;
            let Phase::Client{ticket,..}=&e.phase else {unreachable!()};
            ensure!(state.pairing.remaining_ms(&ticket.target.device_id,generation).is_some(),"Pairing cancelled"); Self::finish(e)
        })
    }
    fn start_delivery(&self, generation: &str) -> Result<bool> {
        self.with(|entries| {
            let Some(entry) = entries.values_mut().find(
                |e| matches!(&e.phase,Phase::Client{ticket,..} if ticket.generation==generation),
            ) else {
                return Ok(false);
            };
            let Phase::Client {
                delivery_started, ..
            } = &mut entry.phase
            else {
                unreachable!()
            };
            let first = !*delivery_started;
            *delivery_started = true;
            Ok(first)
        })
    }
    pub fn process(&self, state: &CoreRuntime, verification: Verification) -> Result<Action> {
        let _commit = state.pairing_runtime.commit.lock().unwrap();
        self.with(|entries| {
            let (signature, proof, ack) = match verification {
                Verification::Finish {
                    request_signature,
                    proof,
                } => (request_signature, proof, false),
                Verification::Ack {
                    request_signature,
                    proof,
                } => (request_signature, proof, true),
            };
            let e = entries
                .get_mut(&signature)
                .ok_or_else(|| anyhow::anyhow!("Pairing expired"))?;
            match &mut e.phase {
                Phase::Server {
                    target,
                    channel,
                    facts,
                    ack: cached,
                    ..
                } if !ack => {
                    let c = channel
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("Pairing approval required"))?;
                    c.verify(&proof, false)?;
                    if cached.is_none() {
                        ensure!(
                            e.started.elapsed() < RESPONSE_TIMEOUT
                                && state
                                    .pairing
                                    .incoming_target(&e.request.from_id, &signature)
                                    .is_some(),
                            "Pairing expired or cancelled"
                        );
                        let receipt = c.proof(true)?;
                        let peer = peer_store::save_confirmed(
                            &state.db,
                            facts
                                .as_ref()
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("Pairing already consumed"))?,
                            e.previous_identity.as_deref(),
                            true,
                        )?;
                        *cached = Some(receipt);
                        state.pairing.take_incoming(&e.request.from_id, &signature);
                        pairing_runtime::success(
                            state,
                            &serde_json::to_value(
                                crate::features::services::peers::peer_address::view(&peer),
                            )?,
                            &target.device_ip,
                        );
                    }
                    Ok(Action::Notify {
                        request: e.request.clone(),
                        target: target.clone(),
                        message: Message::PairVerification(Verification::Ack {
                            request_signature: signature,
                            proof: cached.clone().unwrap(),
                        }),
                    })
                }
                Phase::Client {
                    ticket,
                    channel,
                    facts,
                    finish,
                    completed,
                    ..
                } if ack => {
                    ensure!(finish.is_some(), "Pairing not confirmed");
                    channel
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("Pairing response required"))?
                        .verify(&proof, true)?;
                    if !*completed {
                        ensure!(
                            e.started.elapsed() < RESPONSE_TIMEOUT
                                && state
                                    .pairing
                                    .remaining_ms(&ticket.target.device_id, &ticket.generation)
                                    .is_some(),
                            "Pairing expired or cancelled"
                        );
                        let peer = peer_store::save_confirmed(
                            &state.db,
                            facts
                                .as_ref()
                                .cloned()
                                .ok_or_else(|| anyhow::anyhow!("Pairing already consumed"))?,
                            e.previous_identity.as_deref(),
                            true,
                        )?;
                        *completed = true;
                        state.pairing.take_response(
                            &ticket.target.device_id,
                            Some(&ticket.generation),
                            &signature,
                        );
                        state.pairing_runtime.attempts.transition(
                            state,
                            &ticket.generation,
                            PairingPhase::Succeeded,
                            None,
                            None,
                        );
                        pairing_runtime::success(
                            state,
                            &serde_json::to_value(
                                crate::features::services::peers::peer_address::view(&peer),
                            )?,
                            &ticket.target.device_ip,
                        );
                    }
                    Ok(Action::Done)
                }
                _ => anyhow::bail!("Unexpected pairing confirmation role"),
            }
        })
    }
}
pub async fn deliver(state: &CoreRuntime, action: Action) -> Result<()> {
    match action {
        Action::Notify {
            request,
            target,
            message,
        } => pairing_runtime::send_incoming(state, &request, &target, message).await?,
        Action::Send { ticket, message } => {
            if state
                .pairing_runtime
                .confirmations
                .start_delivery(&ticket.generation)?
            {
                let state = state.clone();
                tokio::spawn(async move {
                    let mut stop = state.stop.clone();
                    while state
                        .pairing
                        .remaining_ms(&ticket.target.device_id, &ticket.generation)
                        .is_some()
                    {
                        if let Err(error) = send_finish(&state, &ticket, &message).await {
                            log::warn!("Pairing confirmation delivery failed: {error}");
                        }
                        tokio::select! {_=tokio::time::sleep(Duration::from_secs(2))=>{},_=stop.changed()=>break}
                    }
                });
            }
        }
        Action::Done => {}
    }
    Ok(())
}
async fn send_finish(state: &CoreRuntime, ticket: &Ticket, message: &Message) -> Result<()> {
    if ticket.target.device_ip.is_empty() {
        let packet =
            crate::infrastructure::protocols::ble::message(&[], message.wire()?.as_bytes())?;
        ensure!(
            state
                .host
                .call(
                    "blePairSend",
                    serde_json::json!({"generation":ticket.generation,"packet":base64_encode(&packet)})
                )
                .await
                .map_err(anyhow::Error::msg)?
                == serde_json::Value::Bool(true),
            "Failed to send pairing confirmation"
        );
    } else {
        ensure!(
            crate::features::discovery::nearby_http::send(
                &ticket.target.device_ip,
                ticket.target.device_port,
                message,
                None,
            )
            .await?,
            "Failed to send pairing confirmation"
        );
    }
    Ok(())
}
pub async fn handle(state: &CoreRuntime, verification: Verification) -> Result<()> {
    let action = state
        .pairing_runtime
        .confirmations
        .process(state, verification)?;
    deliver(state, action).await
}
pub async fn confirm(state: &CoreRuntime, generation: &str) -> Result<()> {
    if state
        .pairing_runtime
        .attempts
        .get(generation)
        .is_some_and(|s| {
            s.mode == crate::features::services::discovery::pairing_attempts::Mode::Manage
        })
    {
        return state.pairing_runtime.attempts.trust(generation);
    }
    let action = state
        .pairing_runtime
        .confirmations
        .confirm(state, generation)?;
    state.pairing_runtime.attempts.transition(
        state,
        generation,
        PairingPhase::Authenticating,
        None,
        None,
    );
    deliver(state, action).await
}
pub fn publish(state: &CoreRuntime, request: PairingRequest) -> Result<()> {
    let changed = crate::infrastructure::db::chat_store::peers::get(&state.db, &request.from_id)?
        .is_some_and(|p| p.public_key != request.payload.signature_public_key);
    let mut value = serde_json::to_value(&request)?;
    value["identityChanged"] = serde_json::json!(changed);
    value["verificationCode"] = serde_json::json!(identity_digest(&request)?);
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
