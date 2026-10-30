use crate::assembly::server::ServerState;
use crate::services::discovery::pairing;
use crate::services::discovery::pairing::Device;
use plain_rs::discovery::nearby_http;
use plain_rs::discovery::nearby_http::Message;
use plain_rs::peers::pairing::protocol::PairingCancel;
use plain_rs::peers::pairing::protocol::PairingRequest;
use plain_rs::peers::pairing::protocol::PairingResponse;
use plain_rs::peers::pairing::sessions::Target;
use plain_rs::peers::pairing::sessions::Ticket;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Mutex,
    time::{Duration, Instant},
};
struct Association {
    signature: String,
    address: String,
    received: Instant,
}
#[derive(Default)]
pub(crate) struct Runtime {
    associations: Mutex<HashMap<String, Association>>,
    pub(crate) confirmations: super::pairing_confirmation::Confirmations,
    pub(crate) attempts: super::pairing_attempts::Attempts,
    pub(crate) commit: Mutex<()>,
    pending_success: Mutex<PendingSuccess>,
}
#[derive(Default)]
struct PendingSuccess {
    requests: BTreeMap<String, String>,
    running: bool,
}
impl Runtime {
    fn check_address(&self, request: &PairingRequest, address: &str) -> anyhow::Result<()> {
        anyhow::ensure!(
            !address.is_empty() && address.len() <= 256,
            "Invalid BLE sender address"
        );
        let mut map = self.associations.lock().unwrap();
        map.retain(|_, v| v.received.elapsed() < Duration::from_secs(300));
        anyhow::ensure!(
            map.contains_key(&request.from_id) || map.len() < 128,
            "Incoming BLE pairing capacity exceeded"
        );
        Ok(())
    }
    fn remember(&self, request: &PairingRequest, address: &str) -> anyhow::Result<()> {
        anyhow::ensure!(
            !address.is_empty() && address.len() <= 256,
            "Invalid BLE sender address"
        );
        let mut map = self.associations.lock().unwrap();
        map.retain(|_, v| v.received.elapsed() < Duration::from_secs(300));
        anyhow::ensure!(
            map.contains_key(&request.from_id) || map.len() < 128,
            "Incoming BLE pairing capacity exceeded"
        );
        map.insert(
            request.from_id.clone(),
            Association {
                signature: request.signature.clone(),
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
    fn take(&self, id: &str, signature: &str) -> Option<String> {
        let mut map = self.associations.lock().unwrap();
        if !map.get(id).is_some_and(|a| a.signature == signature) {
            return None;
        }
        map.remove(id)
            .filter(|a| a.received.elapsed() < Duration::from_secs(300))
            .map(|a| a.address)
    }
    fn forget(&self, id: &str) {
        self.associations.lock().unwrap().remove(id);
    }
}
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum Request {
    States,
    Start {
        target: Target,
        ips: Vec<String>,
        methods: Vec<String>,
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
        request: PairingRequest,
        address: String,
        ble: bool,
    },
    ReceiveCancel {
        cancel: PairingCancel,
    },
    Complete {
        response: PairingResponse,
        sender_ip: String,
    },
    Respond {
        request: PairingRequest,
        accepted: bool,
        device: Device,
    },
}
pub(crate) fn event(state: &ServerState, kind: &'static str, value: Value) {
    let _ = state.events.send(plain_rs::events::WsEvent::broadcast(
        kind,
        value.to_string(),
    ));
}
pub(crate) fn failed(state: &ServerState, target: &Target, error: &str, generation: Option<&str>) {
    let mut result =
        json!({"deviceId":target.device_id,"deviceName":target.device_name,"error":error});
    if let Some(generation) = generation {
        result["generation"] = json!(generation);
    }
    if let Some(generation) = generation {
        state
            .pairing_runtime
            .attempts
            .transition(state, generation, "FAILED", Some(error), None);
    }
    event(state, plain_rs::chat::events::WS_PAIRING_FAILED, result);
}
fn canceled(state: &ServerState, value: &Value) {
    let ticket = &value["ticket"];
    event(
        state,
        plain_rs::chat::events::WS_PAIRING_CANCELLED,
        json!({"deviceId":ticket["deviceId"],"deviceName":ticket["deviceName"]}),
    );
}
pub(crate) fn success(state: &ServerState, peer: &Value, ip: &str) {
    event(
        state,
        plain_rs::chat::events::WS_PAIRING_SUCCESS,
        json!({"deviceId":peer["id"],"deviceName":peer["name"],"ip":ip}),
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

async fn deliver_successes(state: ServerState) {
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
pub(super) async fn local_device(state: &ServerState) -> anyhow::Result<Device> {
    let reply: plain_rs::discovery::nearby_wire::DiscoverReply = serde_json::from_value(
        crate::services::discovery::discovery_advertisement::execute(
            state,
            crate::services::discovery::discovery_advertisement::Request::Reply {},
        )
        .await?,
    )?;
    Ok(Device {
        name: reply.name,
        port: reply.port,
        device_type: reply.device_type.to_string(),
        ips: reply.ips,
        aware_supported: reply.aware_supported,
    })
}
pub(crate) async fn start_device(
    state: &ServerState,
    target: Target,
    ips: Vec<String>,
    methods: Vec<String>,
    generation: Option<String>,
) -> anyhow::Result<Value> {
    let ble = if methods.iter().any(|method| method == "BLE")
        && !(methods.iter().any(|method| method == "LAN") && !ips.is_empty())
    {
        state
            .host
            .call("peerTransportCapabilities", json!({}))
            .await
            .map_err(anyhow::Error::msg)?
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
pub(crate) async fn respond_device(
    state: &ServerState,
    request: PairingRequest,
    accepted: bool,
) -> anyhow::Result<Value> {
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
pub(crate) async fn execute(state: &ServerState, request: Request) -> anyhow::Result<Value> {
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
            if methods.iter().any(|m| m == "LAN") && !ips.is_empty() {
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
            } else if methods.iter().any(|m| m == "BLE") && ble {
                crate::services::discovery::ble_pairing::start(state, target, device, generation)?
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
            target.device_ip = plain_rs::transport::lan::best_host(
                &ips,
                &plain_rs::transport::lan::local_interfaces(),
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
                        .is_none_or(|s| s.phase == "CANCELLED")
                }) {
                    return Ok(Value::Null);
                }
                pairing::start_generation(&state.prefs, &state.pairing, target, device, generation)?
            };
            let ticket: Ticket = serde_json::from_value(result["ticket"].clone())?;
            let request: PairingRequest = serde_json::from_value(result["request"].clone())?;
            let verification = state
                .pairing_runtime
                .confirmations
                .begin(&request, &ticket)?;
            let sent = nearby_http::send(
                &ticket.target.device_ip,
                ticket.target.device_port,
                &Message::PairRequest(request),
            )
            .await
            .unwrap_or(false);
            if sent {
                if state
                    .pairing
                    .mark_sent(&ticket.target.device_id, &ticket.generation)
                {
                    state.pairing_runtime.attempts.transition(
                        state,
                        &ticket.generation,
                        "PAIRING",
                        None,
                        None,
                    );
                    event(
                        state,
                        plain_rs::chat::events::WS_PAIRING_STARTED,
                        json!({"deviceId":ticket.target.device_id,"deviceName":ticket.target.device_name,"generation":ticket.generation}),
                    );
                }
                anyhow::ensure!(
                    nearby_http::send(
                        &ticket.target.device_ip,
                        ticket.target.device_port,
                        &verification
                    )
                    .await?,
                    "Failed to send pairing verification"
                );
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
                let ticket: Ticket = serde_json::from_value(value["ticket"].clone())?;
                let cancel: PairingCancel = serde_json::from_value(value["cancel"].clone())?;
                if !ticket.target.device_ip.is_empty() {
                    let _ = nearby_http::send(
                        &ticket.target.device_ip,
                        ticket.target.device_port,
                        &Message::PairCancel(cancel),
                    )
                    .await;
                }
            }
            value
        }
        Request::ReceiveRequest {
            mut request,
            address,
            ble,
        } => {
            if !ble {
                request.from_ip = address.clone();
            }
            if !plain_rs::peers::pairing::security::verify_request(&request) {
                return Ok(Value::Null);
            }
            if ble {
                state.pairing_runtime.check_address(&request, &address)?;
            }
            let is_new = pairing::receive_request(&state.pairing, &request).unwrap_or(false);
            if ble
                && state
                    .pairing
                    .incoming_target(&request.from_id, &request.signature)
                    .is_some()
            {
                if let Err(error) = state.pairing_runtime.remember(&request, &address) {
                    if is_new {
                        state
                            .pairing
                            .take_incoming(&request.from_id, &request.signature);
                    }
                    return Err(error);
                }
            }
            if is_new {
                state.pairing_runtime.confirmations.register(&request)?;
            }
            json!(is_new)
        }
        Request::ReceiveCancel { cancel } => {
            let id = cancel.from_id.clone();
            let result = pairing::receive_cancel(&state.prefs, &state.pairing, cancel)?;
            if !result.is_null() {
                state.pairing_runtime.forget(&id);
                event(
                    state,
                    plain_rs::chat::events::WS_PAIRING_CANCELLED,
                    result.clone(),
                );
            }
            result
        }
        Request::Complete {
            response,
            sender_ip,
        } => {
            let _commit = state.pairing_runtime.commit.lock().unwrap();
            if state
                .pairing
                .tickets()
                .into_iter()
                .find(|t| t.target.device_id == response.from_id)
                .is_some_and(|t| {
                    state
                        .pairing_runtime
                        .attempts
                        .get(&t.generation)
                        .is_some_and(|s| s.terminal())
                })
            {
                return Ok(Value::Null);
            }
            state.pairing_runtime.confirmations.response(&response)?;
            let result = pairing::complete(
                &state.db,
                &state.prefs,
                &state.pairing,
                response,
                &sender_ip,
            )?;
            if !result.is_null() {
                if result["peer"].is_null() {
                    let ticket: Ticket = serde_json::from_value(result["ticket"].clone())?;
                    failed(
                        state,
                        &ticket.target,
                        result["error"].as_str().unwrap_or("Pairing failed"),
                        Some(&ticket.generation),
                    );
                } else {
                    if let Some(generation) = result["ticket"]["generation"].as_str() {
                        state.pairing_runtime.attempts.transition(
                            state,
                            generation,
                            "SUCCEEDED",
                            None,
                            None,
                        );
                    }
                    success(state, &result["peer"], &sender_ip);
                }
            }
            result
        }
        Request::Respond {
            request,
            accepted,
            device,
        } => {
            let target = state
                .pairing
                .incoming_target(&request.from_id, &request.signature);
            state.pairing_runtime.confirmations.ready(&request)?;
            let mut result = pairing::respond(
                &state.db,
                &state.prefs,
                &state.pairing,
                request.clone(),
                accepted,
                device,
            )?;
            if result.is_null() {
                return Ok(Value::Null);
            }
            let mut response: PairingResponse = serde_json::from_value(result["response"].clone())?;
            state.pairing_runtime.confirmations.seal(&mut response)?;
            result["response"] = serde_json::to_value(&response)?;
            let target = target.ok_or_else(|| anyhow::anyhow!("Incoming pairing disappeared"))?;
            if !result["peer"].is_null() {
                success(state, &result["peer"], &target.device_ip);
            } else {
                failed(state, &target, "Pairing request was rejected", None);
            }
            send_incoming(state, &request, &target, Message::PairResponse(response)).await?;
            state
                .pairing_runtime
                .take(&request.from_id, &request.signature);
            result
        }
    })
}

pub(crate) async fn handle_nearby_post(
    state: &ServerState,
    body: &str,
    remote_ip: &str,
) -> anyhow::Result<bool> {
    use plain_rs::discovery::nearby_wire::Message;
    let kind = body
        .split_once(':')
        .map(|(kind, _)| kind)
        .unwrap_or_default();
    if !matches!(
        kind,
        "PAIR_REQUEST"
            | "PAIR_RESPONSE"
            | "PAIR_CANCEL"
            | "PAIR_VERIFICATION"
            | "DISCOVER"
            | "DISCOVER_REPLY"
    ) {
        return Ok(false);
    }
    match Message::parse(body)? {
        Message::PairRequest(request) => {
            execute(
                state,
                Request::ReceiveRequest {
                    request,
                    address: remote_ip.to_owned(),
                    ble: false,
                },
            )
            .await?;
        }
        Message::PairResponse(response) => {
            execute(
                state,
                Request::Complete {
                    response,
                    sender_ip: remote_ip.to_owned(),
                },
            )
            .await?;
        }
        Message::PairVerification(verification) => {
            super::pairing_confirmation::handle(state, verification).await?;
        }
        Message::PairCancel(cancel) => {
            execute(state, Request::ReceiveCancel { cancel }).await?;
        }
        Message::Discover | Message::DiscoverReply(_) => {}
    }
    Ok(true)
}
pub(crate) async fn accept_report(
    state: &ServerState,
    kind: &str,
    payload: &str,
) -> anyhow::Result<bool> {
    let request = match kind {
        "PAIRING_REQUEST_REPORTED" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Report {
                request: PairingRequest,
                address: String,
            }
            let report: Report = serde_json::from_str(payload)?;
            Request::ReceiveRequest {
                request: report.request,
                address: report.address,
                ble: true,
            }
        }
        "PAIRING_RESPONSE_REPORTED" => Request::Complete {
            response: serde_json::from_str(payload)?,
            sender_ip: String::new(),
        },
        "PAIRING_CANCEL_REPORTED" => Request::ReceiveCancel {
            cancel: serde_json::from_str(payload)?,
        },
        _ => return Ok(false),
    };
    execute(state, request).await?;
    Ok(true)
}
#[cfg(test)]
#[path = "../../../tests/unit/services/discovery/pairing_runtime.rs"]
mod tests;

pub(crate) async fn send_incoming(
    state: &ServerState,
    request: &PairingRequest,
    target: &Target,
    message: Message,
) -> anyhow::Result<()> {
    let address = state
        .pairing_runtime
        .address(&request.from_id, &request.signature);
    let body = message.wire()?;
    let lan = async {
        if target.device_ip.is_empty() {
            return Ok(false);
        }
        nearby_http::send(&target.device_ip, target.device_port, &message).await
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
                .map_err(anyhow::Error::msg),
            None => Ok(false),
        }
    };
    let (lan, ble) = tokio::join!(lan, ble);
    if matches!(&lan, Ok(true)) || matches!(&ble, Ok(true)) {
        return Ok(());
    }
    anyhow::bail!("Failed to send pairing message: LAN {lan:?}, BLE {ble:?}")
}
