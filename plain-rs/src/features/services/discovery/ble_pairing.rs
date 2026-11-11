use crate::features::chat::events::WS_PAIRING_STARTED;
use crate::features::discovery::nearby_wire::Message;
use crate::features::peers::pairing::sessions::Target;
use crate::features::peers::pairing::sessions::Ticket;
use crate::features::services::discovery::pairing;
use crate::features::services::discovery::pairing::Device;
use crate::features::services::discovery::pairing_attempts::PairingPhase;
use crate::features::services::discovery::pairing_runtime::event;
use crate::features::services::discovery::pairing_runtime::failed;
use crate::runtime::Runtime as CoreRuntime;
use plain_libs::utils::base64::base64_encode;
use plain_libs::serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};
#[derive(Default)]
pub struct Pairings(Mutex<HashMap<String, Ticket>>);
enum Cause {
    Stopped,
    Failed(String),
}
pub fn start(
    state: &CoreRuntime,
    mut target: Target,
    device: Device,
    generation: Option<String>,

) -> plain_libs::anyhow::Result<Value> {
    let mut jobs = state.ble_pairing.0.lock().unwrap();
    if let Some(ticket) = jobs.get(&target.device_id) {
        if state
            .pairing
            .remaining_ms(&ticket.target.device_id, &ticket.generation)
            .is_some()
        {
            return Ok(json!({"ticket":ticket}));
        }
        failed(state, &target, "Previous BLE connection is closing", None);
        return Ok(Value::Null);
    }
    if jobs.len() >= 2 {
        failed(state, &target, "BLE pairing capacity exceeded", None);
        return Ok(Value::Null);
    }
    target.device_ip.clear();
    let value = {
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
    let ticket: Ticket = plain_libs::serde_json::from_value(value["ticket"].clone())?;
    let request = plain_libs::serde_json::from_value(value["request"].clone())?;
    state
        .pairing_runtime
        .confirmations
        .begin(state, &request, &ticket)?;
    let body = Message::PairRequest(request.payload).wire()?;
    let packet = crate::infrastructure::protocols::ble::message(
        request.from_id.as_bytes(),
        body.as_bytes(),
    )?;
    let packet = base64_encode(&packet);
    jobs.insert(ticket.target.device_id.clone(), ticket.clone());
    drop(jobs);
    let state = state.clone();
    let returned = ticket.clone();
    tokio::spawn(async move {
        let result = run(&state, &ticket, &packet).await;
        if let Err(Cause::Failed(error)) = result {
            if state
                .pairing
                .remaining_ms(&ticket.target.device_id, &ticket.generation)
                .is_some()
                && state
                    .pairing
                    .cancel(&ticket.target.device_id, Some(&ticket.generation))
                    .is_some()
            {
                failed(&state, &ticket.target, &error, Some(&ticket.generation));
            }
        }
        let _ = state
            .host
            .call("blePairClose", json!({"generation":ticket.generation}))
            .await;
        let mut jobs = state.ble_pairing.0.lock().unwrap();
        if jobs
            .get(&ticket.target.device_id)
            .is_some_and(|t| t.generation == ticket.generation)
        {
            jobs.remove(&ticket.target.device_id);
        }
    });
    Ok(json!({"ticket":returned}))
}
async fn host(
    state: &CoreRuntime,
    ticket: &Ticket,
    method: &str,
    params: Value,
) -> Result<Value, Cause> {
    let call = state.host.call(method, params);
    tokio::pin!(call);
    let mut stop = state.stop.clone();
    let mut timer = tokio::time::interval(Duration::from_millis(100));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _=stop.changed()=>return Err(Cause::Stopped),
            _=timer.tick()=>{if *stop.borrow() || state.pairing.remaining_ms(&ticket.target.device_id,&ticket.generation).is_none() {return Err(Cause::Stopped);}},
            value=&mut call=>return value.map_err(Cause::Failed)
        }
    }
}
async fn run(state: &CoreRuntime, ticket: &Ticket, packet: &str) -> Result<(), Cause> {
    let facts = json!({"generation":ticket.generation});
    let connect = json!({
        "generation": ticket.generation,
        "clientId": ticket.target.device_id,
    });
    if host(state, ticket, "blePairConnect", connect).await? != Value::Bool(true) {
        return Err(Cause::Failed("Connection failed".into()));
    }
    let mut send = facts.clone();
    send["packet"] = json!(packet);
    if host(state, ticket, "blePairSend", send.clone()).await? != Value::Bool(true) {
        return Err(Cause::Failed("Failed to send pairing request".into()));
    }
    if !state
        .pairing
        .mark_sent(&ticket.target.device_id, &ticket.generation)
    {
        return Err(Cause::Stopped);
    }
    state.pairing_runtime.attempts.transition(
        state,
        &ticket.generation,
        PairingPhase::Pairing,
        None,
        None,
    );
    event(
        state,
        WS_PAIRING_STARTED,
        json!({"deviceId":ticket.target.device_id,"deviceName":ticket.target.device_name,"generation":ticket.generation}),
    );
    let mut last_send = Instant::now();
    loop {
        let Some(remaining) = state
            .pairing
            .remaining_ms(&ticket.target.device_id, &ticket.generation)
        else {
            return Err(Cause::Stopped);
        };
        if last_send.elapsed() >= Duration::from_secs(3)
            && state
                .pairing_runtime
                .confirmations
                .awaiting_response(state, ticket)
        {
            if host(state, ticket, "blePairSend", send.clone()).await? != Value::Bool(true) {
                return Err(Cause::Failed("Failed to resend pairing request".into()));
            }
            last_send = Instant::now();
        }
        let mut wait = facts.clone();
        wait["timeoutMs"] = json!(remaining.min(1000));
        let response = host(state, ticket, "blePairWait", wait).await?;
        if response.get("connected").and_then(Value::as_bool) != Some(true) {
            return Err(Cause::Failed("Connection lost".into()));
        }
        let Some(body) = response.get("notification").and_then(Value::as_str) else {
            continue;
        };
        if body.len() > 65536 {
            continue;
        }
        let message = match Message::parse(body) {
            Ok(message) => message,
            Err(_) => continue,
        };
        if let Message::PairResponse(response) = message {
            if response.request_id == crate::features::peers::pairing::protocol::request_id(&ticket.request_signature) {
                match state
                    .pairing_runtime
                    .confirmations
                    .response(state, &response, "")
                {
                    Ok(()) => return Ok(()),
                    Err(error) => log::warn!("BLE pairing response rejected: {error}"),
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/discovery/ble_pairing.rs"]
mod tests;
