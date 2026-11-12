use crate::features::discovery::nearby_devices::DiscoveryMethod;
use crate::features::discovery::nearby_http;
use crate::features::discovery::nearby_http::Message;
use crate::features::peers::pairing::protocol::PairingCancel;
use crate::features::peers::pairing::protocol::PairingReply;
use crate::features::peers::pairing::protocol::{PairingRequest, PairingRequestPayload};
use crate::features::peers::pairing::sessions::Target;
use crate::features::peers::pairing::sessions::Ticket;
use crate::features::services::discovery::pairing;
use crate::features::services::discovery::pairing::Device;
use crate::features::services::discovery::pairing_attempts::PairingPhase;
use crate::runtime::Runtime as CoreRuntime;
use serde::Deserialize;
use plain_libs::serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap},
    net::IpAddr,
    sync::Mutex,
    time::{Duration, Instant},
};
struct Association {
    signature: String,
    address: String,
    received: Instant,
}
#[derive(Default)]
pub struct Runtime {
    pub qr: super::pairing_qr::Pairings,
    associations: Mutex<HashMap<String, Association>>,
    pub confirmations: crate::features::services::discovery::pairing_confirmation::Confirmations,
    pub attempts: crate::features::services::discovery::pairing_attempts::Attempts,
    pub commit: Mutex<()>,
    pending_success: Mutex<PendingSuccess>,
}
#[derive(Default)]
struct PendingSuccess {
    requests: BTreeMap<String, String>,
    running: bool,
}
impl Runtime {
    fn check_address(&self, request: &PairingRequest, address: &str) -> plain_libs::anyhow::Result<()> {
        plain_libs::anyhow::ensure!(
            !address.is_empty() && address.len() <= 256,
            "Invalid BLE sender address"
        );
        let mut map = self.associations.lock().unwrap();
        map.retain(|_, v| v.received.elapsed() < Duration::from_secs(300));
        plain_libs::anyhow::ensure!(
            map.contains_key(&request.from_id) || map.len() < 128,
            "Incoming BLE pairing capacity exceeded"
        );
        Ok(())
    }
    fn remember(&self, request: &PairingRequest, address: &str) -> plain_libs::anyhow::Result<()> {
        plain_libs::anyhow::ensure!(
            !address.is_empty() && address.len() <= 256,
            "Invalid BLE sender address"
        );
        let mut map = self.associations.lock().unwrap();
        map.retain(|_, v| v.received.elapsed() < Duration::from_secs(300));
        plain_libs::anyhow::ensure!(
            map.contains_key(&request.from_id) || map.len() < 128,
            "Incoming BLE pairing capacity exceeded"
        );
        map.insert(
            request.from_id.clone(),
            Association {
                signature: request.payload.signature.clone(),
                address: address.into(),
                received: Instant::now(),
            },
        );
        Ok(())
    }
    fn address(&self, id: &str, signature: &str) -> Option<String> {
        self.associations
            .lock()
            .unwrap()
            .get(id)
            .filter(|a| a.signature == signature && a.received.elapsed() < Duration::from_secs(300))
            .map(|a| a.address.clone())
    }
    fn forget(&self, id: &str) {
        self.associations.lock().unwrap().remove(id);
    }
}
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "camelCase", deny_unknown_fields)]
pub enum Request {
    States,
    Start {
        target: Target,
        ips: Vec<String>,
        methods: Vec<DiscoveryMethod>,
        ble: bool,
        device: Device,
        generation: Option<String>,
    },
    StartLan {
        target: Target,
        ips: Vec<String>,
        device: Device,
        generation: Option<String>,
    },
    Cancel {
        id: String,
        generation: Option<String>,
    },
    ReceiveRequest {
        payload: PairingRequestPayload,
        client_id: String,
        address: String,
        ble: bool,
    },
    ReceiveCancel {
        cancel: PairingCancel,
    },
    Complete {
        response: PairingReply,
        sender_ip: String,
    },
    Respond {
        request: PairingRequest,
        accepted: bool,
        device: Device,
    },
}
pub fn event(state: &CoreRuntime, kind: &'static str, value: Value) {
    let _ = state
        .events
        .send(crate::infrastructure::events::WsEvent::broadcast(
            kind,
            value.to_string(),
        ));
}
pub fn failed(state: &CoreRuntime, target: &Target, error: &str, generation: Option<&str>) {
    let mut result =
        json!({"deviceId":target.device_id,"deviceName":target.device_name,"error":error});
    if let Some(generation) = generation {
        result["generation"] = json!(generation);
    }
    if let Some(generation) = generation {
        state.pairing_runtime.attempts.transition(
            state,
            generation,
            PairingPhase::Failed,
            Some(error),
            None,
        );
    }
    event(
        state,
        crate::features::chat::events::WS_PAIRING_FAILED,
        result,
    );
}
fn canceled(state: &CoreRuntime, value: &Value) {
    let ticket = &value["ticket"];
    event(
        state,
        crate::features::chat::events::WS_PAIRING_CANCELLED,
        json!({"deviceId":ticket["deviceId"],"deviceName":ticket["deviceName"]}),
    );
}
pub fn success(state: &CoreRuntime, peer: &Value, ip: &str, request_id: Option<&str>) {
    event(
        state,
        crate::features::chat::events::WS_PAIRING_SUCCESS,
        json!({"deviceId":peer["id"],"deviceName":peer["name"],"ip":ip,"port":peer["port"],"deviceType":peer["device_type"],"requestId":request_id}),
    );
    let (Some(id), Some(key)) = (peer["id"].as_str(), peer["key"].as_str()) else {
        log::error!("Pairing result is missing its identity");
        return;
    };
    let mut pending = state.pairing_runtime.pending_success.lock().unwrap();
    pending.requests.insert(id.to_owned(), key.to_owned());
    if pending.running {
        return;
    }
    pending.running = true;
    let state = state.clone();
    tokio::spawn(async move { deliver_successes(state).await });
}

async fn deliver_successes(state: CoreRuntime) {
    let mut connection = state.host.connection_changes();
    let mut stop = state.stop.clone();
    loop {
        let next = {
            let mut pending = state.pairing_runtime.pending_success.lock().unwrap();
            match pending.requests.iter().next() {
                Some((id, key)) => Some((id.clone(), key.clone())),
                None => {
                    pending.running = false;
                    None
                }
            }
        };
        let Some((id, key)) = next else { return };
        let Some(generation) = state.host.generation() else {
            tokio::select! {
                _ = stop.changed() => break,
                result = connection.changed() => if result.is_err() { break },
            }
            continue;
        };
        let permit = tokio::select! {
            _ = stop.changed() => break,
            result = connection.changed() => { if result.is_err() { break; } continue; },
            result = state.host.reserve_callback() => match result { Ok(permit) => permit, Err(_) => break },
        };
        let response = tokio::select! {
            _ = stop.changed() => break,
            result = state.host.call_reserved_current(generation, "pairingSucceeded", json!({"deviceId":id,"key":key}), permit) => result,
        };
        match response {
            Ok(_) => {
                let mut pending = state.pairing_runtime.pending_success.lock().unwrap();
                if pending.requests.get(&id) == Some(&key) {
                    pending.requests.remove(&id);
                }
            }
            Err(error) => {
                log::warn!("Pairing host callback failed: {error}");
                if state.host.is_current(generation) {
                    tokio::select! {
                        _ = stop.changed() => break,
                        result = connection.changed() => if result.is_err() { break },
                    }
                }
            }
        }
    }
    state
        .pairing_runtime
        .pending_success
        .lock()
        .unwrap()
        .running = false;
}
pub(super) async fn local_device(state: &CoreRuntime) -> plain_libs::anyhow::Result<Device> {
    let reply: crate::features::discovery::nearby_wire::DiscoverReply = plain_libs::serde_json::from_value(
        crate::features::services::discovery::discovery_advertisement::execute(
            state,
            crate::features::services::discovery::discovery_advertisement::Request::Reply {},
        )
        .await?,
    )?;
    Ok(Device {
        name: reply.name,
        port: reply.port,
        device_type: reply.device_type,
        ips: reply.ips,
        aware_supported: reply.aware_supported,
    })
}
pub async fn start_device(
    state: &CoreRuntime,
    target: Target,
    ips: Vec<String>,
    methods: Vec<DiscoveryMethod>,
    generation: Option<String>,

) -> plain_libs::anyhow::Result<Value> {
    let ble = if methods.iter().any(|method| *method == DiscoveryMethod::Ble)
        && !(methods.iter().any(|method| method.is_lan()) && !ips.is_empty())
    {
        state
            .host
            .call("peerTransportCapabilities", json!({}))
            .await
            .map_err(plain_libs::anyhow::Error::msg)?
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item.as_str() == Some("BLE")))
    } else {
        false
    };
    let device = local_device(state).await?;
    execute(
        state,
        Request::Start {
            target,
            ips,
            methods,
            ble,
            device,
            generation,
        },
    )
    .await
}
pub async fn respond_device(
    state: &CoreRuntime,
    request: PairingRequest,
    accepted: bool,
) -> plain_libs::anyhow::Result<Value> {
    let device = local_device(state).await?;
    execute(
        state,
        Request::Respond {
            request,
            accepted,
            device,
        },
    )
    .await
}
pub async fn execute(state: &CoreRuntime, request: Request) -> plain_libs::anyhow::Result<Value> {
    Ok(match request {
        Request::States => json!(state.pairing_runtime.attempts.states()),
        Request::Start {
            target,
            ips,
            methods,
            ble,
            device,
            generation,
        } => {
            if methods.iter().any(|m| m.is_lan()) && !ips.is_empty() {
                Box::pin(execute(
                    state,
                    Request::StartLan {
                        target,
                        ips,
                        device,
                        generation,
                    },
                ))
                .await?
            } else if methods.iter().any(|m| *m == DiscoveryMethod::Ble) && ble {
                crate::features::services::discovery::ble_pairing::start(
                    state,
                    target,
                    device,
                    generation,
                )?
            } else {
                failed(state, &target, "No pairing transport available", None);
                Value::Null
            }
        }
        Request::StartLan {
            mut target,
            ips,
            device,
            generation,
        } => {
            target.device_ip = crate::infrastructure::transport::lan::best_host(
                &ips,
                &crate::infrastructure::transport::lan::local_interfaces(),
            );
            if target.device_ip.is_empty() {
                failed(state, &target, "No reachable pairing address", None);
                return Ok(Value::Null);
            }
            let result = {
                let _commit = state.pairing_runtime.commit.lock().unwrap();
                if generation.as_ref().is_some_and(|g| {
                    state
                        .pairing_runtime
                        .attempts
                        .get(g)
                        .is_none_or(|s| s.phase == PairingPhase::Cancelled)
                }) {
                    return Ok(Value::Null);
                }
                pairing::start_generation(
                    &state.prefs,
                    &state.pairing,
                    target,
                    device,
                    generation,
                )?
            };
            let ticket: Ticket = plain_libs::serde_json::from_value(result["ticket"].clone())?;
            let request: PairingRequest = plain_libs::serde_json::from_value(result["request"].clone())?;
            state
                .pairing_runtime
                .confirmations
                .begin(state, &request, &ticket)?;
            let message = Message::PairRequest(request.payload);
            let mut sent = false;
            for _ in 0..2 {
                if !state.pairing_runtime.confirmations.awaiting_response(state, &ticket) {
                    break;
                }
                sent = nearby_http::send(
                    &ticket.target.device_ip,
                    ticket.target.device_port,
                    &message,
                    Some(&request.from_id),
                )
                .await
                .unwrap_or(false);
                if sent {
                    break;
                }
            }
            if sent {
                if state
                    .pairing
                    .mark_sent(&ticket.target.device_id, &ticket.generation)
                {
                    state.pairing_runtime.attempts.transition(
                        state,
                        &ticket.generation,
                        PairingPhase::Pairing,
                        None,
                        None,
                    );
                    event(
                        state,
                        crate::features::chat::events::WS_PAIRING_STARTED,
                        json!({"deviceId":ticket.target.device_id,"deviceName":ticket.target.device_name,"generation":ticket.generation}),
                    );
                }
                let state = state.clone();
                let retry_ticket = ticket.clone();
                let sender = request.from_id;
                tokio::spawn(async move {
                    let mut stop = state.stop.clone();
                    loop {
                        tokio::select! {
                            _ = stop.changed() => break,
                            _ = tokio::time::sleep(Duration::from_secs(3)) => {}
                        }
                        if !state
                            .pairing_runtime
                            .confirmations
                            .awaiting_response(&state, &retry_ticket)
                        {
                            break;
                        }
                        let _ = nearby_http::send(
                            &retry_ticket.target.device_ip,
                            retry_ticket.target.device_port,
                            &message,
                            Some(&sender),
                        )
                        .await;
                    }
                });
            } else if state
                .pairing
                .cancel(&ticket.target.device_id, Some(&ticket.generation))
                .is_some()
            {
                failed(
                    state,
                    &ticket.target,
                    "Failed to send pairing request",
                    Some(&ticket.generation),
                );
            }
            json!({"sent":sent,"ticket":ticket})
        }
        Request::Cancel { id, generation } => {
            let value = pairing::cancel(&state.prefs, &state.pairing, &id, generation.as_deref())?;
            if !value.is_null() {
                canceled(state, &value);
                let ticket: Ticket = plain_libs::serde_json::from_value(value["ticket"].clone())?;
                let cancel: PairingCancel = plain_libs::serde_json::from_value(value["cancel"].clone())?;
                if !ticket.target.device_ip.is_empty() {
                    let _ = nearby_http::send(
                        &ticket.target.device_ip,
                        ticket.target.device_port,
                        &Message::PairCancel(cancel),
                        None,
                    )
                    .await;
                }
            }
            value
        }
        Request::ReceiveRequest {
            payload,
            client_id,
            address,
            ble,
        } => {
            plain_libs::anyhow::ensure!(
                !client_id.is_empty() && client_id.len() <= 256,
                "Invalid pairing client ID"
            );
            let local_key =
                crate::features::services::peers::peer_wire::signing_keypair(&state.prefs)?;
            let request = PairingRequest {
                to_id: state.native_client_id().map_err(plain_libs::anyhow::Error::msg)?,
                target_public_key: plain_libs::utils::base64::base64_encode(&local_key[32..]),
                from_id: client_id,
                from_ip: if ble { String::new() } else { address.clone() },
                payload,
            };
            if !crate::features::peers::pairing::security::verify_request(&request) {
                return Ok(Value::Null);
            }
            let known =
                crate::infrastructure::db::chat_store::peers::get(&state.db, &request.from_id)?
                    .filter(|peer| peer.public_key == request.payload.signature_public_key);
            if ble {
                state.pairing_runtime.check_address(&request, &address)?;
            }
            let Some(known) = known else {
                return Ok(Value::Null);
            };
            let (cached, is_new) = {
                let _commit = state.pairing_runtime.commit.lock().unwrap();
                let cached = state.pairing_runtime.confirmations.cached_response(&request)?;
                let is_new = cached.is_none() && state.pairing.receive(&request);
                (cached, is_new)
            };
            if ble
                && (cached.is_some()
                    || state
                        .pairing
                        .incoming_target(&request.from_id, &request.payload.signature)
                        .is_some())
            {
                if let Err(error) = state.pairing_runtime.remember(&request, &address) {
                    if is_new {
                        state
                            .pairing
                            .take_incoming(&request.from_id, &request.payload.signature);
                    }
                    return Err(error);
                }
            }
            if let Some(response) = cached {
                if matches!(response, PairingReply::Accept(_)) && !known.is_paired() {
                    return Ok(Value::Null);
                }
                send_incoming(
                    state,
                    &request,
                    &Target::from(&request),
                    Message::from(response),
                )
                .await?;
            } else if is_new
                && state
                    .pairing_runtime
                    .confirmations
                    .register(state, &request)?
            {
                let pinned = known.is_paired();
                if pinned {
                    Box::pin(respond_device(state, request, true)).await?;
                } else {
                    crate::features::services::discovery::pairing_confirmation::publish(
                        state, request,
                    )?;
                }
            }
            json!(is_new)
        }
        Request::ReceiveCancel { cancel } => {
            let id = cancel.from_id.clone();
            let result = {
                let _commit = state.pairing_runtime.commit.lock().unwrap();
                pairing::receive_cancel(&state.prefs, &state.pairing, cancel)?
            };
            if !result.is_null() {
                state.pairing_runtime.forget(&id);
                event(
                    state,
                    crate::features::chat::events::WS_PAIRING_CANCELLED,
                    result.clone(),
                );
            }
            result
        }
        Request::Complete {
            response,
            sender_ip,
        } => {
            state
                .pairing_runtime
                .confirmations
                .response(state, &response, &sender_ip)?;
            Value::Null
        }
        Request::Respond {
            request,
            accepted,
            device,
        } => {
            let (response, target) = state
                .pairing_runtime
                .confirmations
                .respond(state, &request, accepted, device)?;
            send_incoming(
                state,
                &request,
                &target,
                Message::from(response.clone()),
            )
            .await?;
            json!({"response":response})
        }
    })
}

pub async fn handle_nearby_post(
    state: &CoreRuntime,
    body: &str,
    remote_ip: IpAddr,
    client_id: Option<&str>,
) -> plain_libs::anyhow::Result<bool> {
    use crate::features::discovery::nearby_wire::Message;
    let kind = body
        .split_once(':')
        .map(|(kind, _)| kind)
        .unwrap_or_default();
    if !matches!(
        kind,
        "PAIR_REQUEST" | "PAIRING_ACCEPT" | "PAIRING_REJECT" | "PAIR_CANCEL" | "DISCOVER" | "DISCOVER_REPLY"
    ) {
        return Ok(false);
    }
    match Message::parse(body)? {
        Message::PairRequest(payload) => {
            execute(
                state,
                Request::ReceiveRequest {
                    payload,
                    client_id: client_id.unwrap_or_default().to_owned(),
                    address: remote_ip.to_string(),
                    ble: false,
                },
            )
            .await?;
        }
        Message::PairingAccept(response) if response.signature_public_key.is_some() => {
            super::pairing_qr::complete(state, response, remote_ip, client_id.unwrap_or_default())?;
        }
        Message::PairingAccept(response) => {
            execute(state, Request::Complete { response: PairingReply::Accept(response), sender_ip: remote_ip.to_string() }).await?;
        }
        Message::PairingReject(response) => {
            execute(state, Request::Complete { response: PairingReply::Reject(response), sender_ip: remote_ip.to_string() }).await?;
        }
        Message::PairCancel(cancel) => {
            execute(state, Request::ReceiveCancel { cancel }).await?;
        }
        Message::Discover | Message::DiscoverReply(_) => {}
    }
    Ok(true)
}

pub async fn accept_report(state: &CoreRuntime, kind: &str, payload: &str) -> plain_libs::anyhow::Result<bool> {
    let request = match kind {
        "PAIRING_REQUEST_REPORTED" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Report {
                request: PairingRequest,
                address: String,
            }
            let report: Report = plain_libs::serde_json::from_str(payload)?;
            Request::ReceiveRequest {
                payload: report.request.payload,
                client_id: report.request.from_id,
                address: report.address,
                ble: true,
            }
        }
        "PAIRING_REPLY_REPORTED" => Request::Complete {
            response: plain_libs::serde_json::from_str(payload)?,
            sender_ip: String::new(),
        },
        "PAIRING_CANCEL_REPORTED" => Request::ReceiveCancel {
            cancel: plain_libs::serde_json::from_str(payload)?,
        },
        _ => return Ok(false),
    };
    execute(state, request).await?;
    Ok(true)
}

pub async fn send_incoming(
    state: &CoreRuntime,
    request: &PairingRequest,
    target: &Target,
    message: Message,
) -> plain_libs::anyhow::Result<()> {
    let address = state
        .pairing_runtime
        .address(&request.from_id, &request.payload.signature);
    let body = message.wire()?;
    let lan = async {
        if target.device_ip.is_empty() {
            return Ok(false);
        }
        nearby_http::send(&target.device_ip, target.device_port, &message, None).await
    };
    let ble = async {
        match address {
            Some(address) => state
                .host
                .call(
                    "pairingNotification",
                    json!({"address":address,"body":body}),
                )
                .await
                .map(|value| value == Value::Bool(true))
                .map_err(plain_libs::anyhow::Error::msg),
            None => Ok(false),
        }
    };
    let (lan, ble) = tokio::join!(lan, ble);
    if matches!(&lan, Ok(true)) || matches!(&ble, Ok(true)) {
        return Ok(());
    }
    plain_libs::anyhow::bail!("Failed to send pairing message: LAN {lan:?}, BLE {ble:?}")
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/discovery/pairing_runtime.rs"]
mod tests;
