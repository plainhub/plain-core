use crate::assembly::server::ServerState;
use crate::services::confirmation_channel::{Channel, Client, ConfirmationMode, ServerHandshake};
use anyhow::{Result, ensure};
use plain_rs::discovery::nearby_wire::Message;
use plain_rs::peers::pairing::{
    confirmation::Verification,
    protocol::{PairingRequest, PairingResponse},
    sessions::{RESPONSE_TIMEOUT, Ticket},
};
use plain_rs::utils::base64::base64_encode;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{collections::HashMap, sync::Mutex, time::Instant};

#[derive(Default)]
pub(crate) struct Confirmations(Mutex<HashMap<String, Entry>>);
struct Entry {
    started: Instant,
    phase: Phase,
}
enum Phase {
    Client {
        handshake: Client,
        ticket: Ticket,
    },
    ServerRequest(PairingRequest),
    Server {
        handshake: ServerHandshake,
        request: PairingRequest,
    },
    ClientReady {
        public_key: String,
        channel: Channel,
    },
    ServerReady {
        channel: Channel,
    },
}
pub(crate) enum Action {
    Notify {
        request: PairingRequest,
        message: Message,
    },
    Send {
        ticket: Ticket,
        message: Message,
    },
    Ready {
        request: PairingRequest,
        code: String,
    },
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VerifiedRequest {
    #[serde(flatten)]
    request: PairingRequest,
    verification_code: String,
}
fn context(request: &PairingRequest) -> String {
    base64_encode(&Sha256::digest(request.signature_data().as_bytes()))
}
impl Confirmations {
    fn with<T>(&self, f: impl FnOnce(&mut HashMap<String, Entry>) -> Result<T>) -> Result<T> {
        let mut entries = self.0.lock().unwrap();
        entries.retain(|_, entry| entry.started.elapsed() < RESPONSE_TIMEOUT);
        f(&mut entries)
    }
    pub(crate) fn begin(&self, request: &PairingRequest, ticket: &Ticket) -> Result<Message> {
        let (handshake, hello) = Client::start(&context(request), ConfirmationMode::ChatPairing)?;
        self.with(|entries| {
            entries.insert(
                request.signature.clone(),
                Entry {
                    started: Instant::now(),
                    phase: Phase::Client {
                        handshake,
                        ticket: ticket.clone(),
                    },
                },
            );
            Ok(())
        })?;
        Ok(Message::PairVerification(Verification::Hello {
            from_id: request.from_id.clone(),
            request_signature: request.signature.clone(),
            hello,
        }))
    }
    pub(crate) fn register(&self, request: &PairingRequest) -> Result<()> {
        self.with(|entries| {
            entries.insert(
                request.signature.clone(),
                Entry {
                    started: Instant::now(),
                    phase: Phase::ServerRequest(request.clone()),
                },
            );
            Ok(())
        })
    }
    pub(crate) fn response(&self, response: &PairingResponse) -> Result<()> {
        self.with(|entries| {
            let Some(Phase::ClientReady {
                public_key,
                channel,
            }) = entries
                .get(&response.request_signature)
                .map(|entry| &entry.phase)
            else {
                anyhow::bail!("Pairing verification required");
            };
            ensure!(
                public_key == &response.signature_public_key,
                "Peer identity changed"
            );
            let proof = channel.decrypt_response(
                &plain_rs::utils::base64::base64_decode(&response.confirmation_proof),
                plain_rs::peers::pairing::now_ms() as u64,
            )?;
            ensure!(
                proof == response.signature_data().as_bytes(),
                "Pairing verification failed"
            );
            Ok(())
        })
    }
    pub(crate) fn seal(&self, response: &mut PairingResponse) -> Result<()> {
        self.with(|entries| {
            let Some(Phase::ServerReady { channel }) = entries
                .get(&response.request_signature)
                .map(|entry| &entry.phase)
            else {
                anyhow::bail!("Pairing verification required");
            };
            response.confirmation_proof = base64_encode(&channel.encrypt_response(
                response.signature_data().as_bytes(),
                plain_rs::peers::pairing::now_ms() as u64,
            )?);
            Ok(())
        })
    }
    pub(crate) fn ready(&self, request: &PairingRequest) -> Result<()> {
        self.with(|entries| {
            ensure!(
                matches!(
                    entries.get(&request.signature).map(|entry| &entry.phase),
                    Some(Phase::ServerReady { .. })
                ),
                "Pairing verification required"
            );
            Ok(())
        })
    }
    pub(crate) fn process(
        &self,
        state: &ServerState,
        verification: Verification,
    ) -> Result<Action> {
        self.with(|entries| match verification {
            Verification::Hello {
                from_id,
                request_signature,
                hello,
            } => {
                ensure!(
                    state
                        .pairing
                        .incoming_target(&from_id, &request_signature)
                        .is_some(),
                    "Pairing request expired"
                );
                let entry = entries
                    .remove(&request_signature)
                    .ok_or_else(|| anyhow::anyhow!("Unknown pairing verification"))?;
                let Phase::ServerRequest(request) = entry.phase else {
                    anyhow::bail!("Unexpected pairing verification step");
                };
                let key = crate::services::peers::peer_wire::signing_keypair(&state.prefs)?;
                let id = state.native_client_id().map_err(anyhow::Error::msg)?;
                let (handshake, hello) = ServerHandshake::start(
                    &context(&request),
                    ConfirmationMode::ChatPairing,
                    &id,
                    &hello,
                    &key,
                    plain_rs::peers::pairing::now_ms() as u64,
                )?;
                let message = Message::PairVerification(Verification::Challenge {
                    request_signature: request_signature.clone(),
                    hello,
                    signature_public_key: base64_encode(&key[32..]),
                });
                entries.insert(
                    request_signature,
                    Entry {
                        started: entry.started,
                        phase: Phase::Server {
                            handshake,
                            request: request.clone(),
                        },
                    },
                );
                Ok(Action::Notify { request, message })
            }
            Verification::Challenge {
                request_signature,
                hello,
                signature_public_key,
            } => {
                let entry = entries
                    .remove(&request_signature)
                    .ok_or_else(|| anyhow::anyhow!("Unknown pairing verification"))?;
                let Phase::Client { handshake, ticket } = entry.phase else {
                    anyhow::bail!("Unexpected pairing verification step");
                };
                ensure!(
                    state
                        .pairing
                        .remaining_ms(&ticket.target.device_id, &ticket.generation)
                        .is_some(),
                    "Pairing expired"
                );
                if let Some(saved) =
                    plain_rs::db::chat_store::peers::get(&state.db, &ticket.target.device_id)?
                {
                    ensure!(
                        saved.public_key.is_empty() || saved.public_key == signature_public_key,
                        "Peer identity changed"
                    );
                }
                let (channel, reveal) = handshake.accept(
                    &hello,
                    &signature_public_key,
                    &ticket.target.device_id,
                    plain_rs::peers::pairing::now_ms() as u64,
                )?;
                state.pairing_runtime.attempts.set_verification_code(
                    state,
                    &ticket.generation,
                    &channel.verification_code,
                );
                state.pairing_runtime.attempts.transition(
                    state,
                    &ticket.generation,
                    "WAITING_CONFIRMATION",
                    None,
                    None,
                );
                entries.insert(
                    request_signature.clone(),
                    Entry {
                        started: entry.started,
                        phase: Phase::ClientReady {
                            public_key: signature_public_key,
                            channel,
                        },
                    },
                );
                let message = Message::PairVerification(Verification::Reveal {
                    from_id: state.native_client_id().map_err(anyhow::Error::msg)?,
                    request_signature,
                    reveal,
                });
                Ok(Action::Send { ticket, message })
            }
            Verification::Reveal {
                from_id,
                request_signature,
                reveal,
            } => {
                ensure!(
                    state
                        .pairing
                        .incoming_target(&from_id, &request_signature)
                        .is_some(),
                    "Pairing request expired"
                );
                let entry = entries
                    .remove(&request_signature)
                    .ok_or_else(|| anyhow::anyhow!("Unknown pairing verification"))?;
                let Phase::Server { handshake, request } = entry.phase else {
                    anyhow::bail!("Unexpected pairing verification step");
                };
                let channel =
                    handshake.finish(&reveal, plain_rs::peers::pairing::now_ms() as u64)?;
                let code = channel.verification_code.clone();
                entries.insert(
                    request_signature,
                    Entry {
                        started: entry.started,
                        phase: Phase::ServerReady { channel },
                    },
                );
                Ok(Action::Ready { request, code })
            }
        })
    }
}

pub(crate) async fn handle(state: &ServerState, verification: Verification) -> Result<()> {
    match state
        .pairing_runtime
        .confirmations
        .process(state, verification)?
    {
        Action::Notify { request, message } => {
            let target = state
                .pairing
                .incoming_target(&request.from_id, &request.signature)
                .ok_or_else(|| anyhow::anyhow!("Pairing expired"))?;
            super::pairing_runtime::send_incoming(state, &request, &target, message).await?;
        }
        Action::Send { ticket, message } => {
            ensure!(
                plain_rs::discovery::nearby_http::send(
                    &ticket.target.device_ip,
                    ticket.target.device_port,
                    &message
                )
                .await?,
                "Failed to send pairing verification"
            );
        }
        Action::Ready { request, code } => publish(state, request, code)?,
    }
    Ok(())
}
pub(crate) fn publish(state: &ServerState, request: PairingRequest, code: String) -> Result<()> {
    super::pairing_runtime::event(
        state,
        plain_rs::chat::events::WS_PAIRING_REQUEST_RECEIVED,
        serde_json::to_value(VerifiedRequest {
            request,
            verification_code: code,
        })?,
    );
    Ok(())
}

#[cfg(all(test, feature = "http_transport"))]
#[path = "../../../tests/unit/services/discovery/pairing_confirmation.rs"]
pub(crate) mod tests;
