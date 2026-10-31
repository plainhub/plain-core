use plain_rs::chat::events::WS_PAIRING_STARTED;
use plain_rs::discovery::nearby_wire::Message;
use plain_rs::peers::pairing::sessions::Target;
use plain_rs::peers::pairing::sessions::Ticket;
use plain_rs::runtime::Runtime as CoreRuntime;
use plain_rs::services::discovery::pairing;
use plain_rs::services::discovery::pairing::Device;
use plain_rs::services::discovery::pairing_runtime::event;
use plain_rs::services::discovery::pairing_runtime::failed;
use plain_rs::services::discovery::pairing_runtime::success;
use serde_json::{Value, json};
use std::{collections::HashMap, sync::Mutex, time::Duration};
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
) -> anyhow::Result<Value> {
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
                .is_none_or(|s| s.phase == "CANCELLED")
        }) {
            return Ok(Value::Null);
        }
        pairing::start_generation(&state.prefs, &state.pairing, target, device, generation)?
    };
    let ticket: Ticket = serde_json::from_value(value["ticket"].clone())?;
    let request = serde_json::from_value(value["request"].clone())?;
    let verification = state
        .pairing_runtime
        .confirmations
        .begin(&request, &ticket)?
        .wire()?;
    let body = Message::PairRequest(request).wire()?;
    jobs.insert(ticket.target.device_id.clone(), ticket.clone());
    drop(jobs);
    let state = state.clone();
    let returned = ticket.clone();
    tokio::spawn(async move {
        let result = run(&state, &ticket, &body, &verification).await;
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
async fn run(
    state: &CoreRuntime,
    ticket: &Ticket,
    body: &str,
    verification: &str,
) -> Result<(), Cause> {
    let facts = json!({"generation":ticket.generation});
    let connect = json!({
        "generation": ticket.generation,
        "shortId": plain_rs::discovery::nearby_wire::short_id(&ticket.target.device_id),
    });
    if host(state, ticket, "blePairConnect", connect).await? != Value::Bool(true) {
        return Err(Cause::Failed("Connection failed".into()));
    }
    let mut send = facts.clone();
    send["body"] = json!(body);
    if host(state, ticket, "blePairSend", send).await? != Value::Bool(true) {
        return Err(Cause::Failed("Failed to send pairing request".into()));
    }
    if !state
        .pairing
        .mark_sent(&ticket.target.device_id, &ticket.generation)
    {
        return Err(Cause::Stopped);
    }
    state
        .pairing_runtime
        .attempts
        .transition(state, &ticket.generation, "PAIRING", None, None);
    event(
        state,
        WS_PAIRING_STARTED,
        json!({"deviceId":ticket.target.device_id,"deviceName":ticket.target.device_name,"generation":ticket.generation}),
    );
    let mut verify = facts.clone();
    verify["body"] = json!(verification);
    if host(state, ticket, "blePairSend", verify).await? != Value::Bool(true) {
        return Err(Cause::Failed("Failed to send pairing verification".into()));
    }
    loop {
        let Some(remaining) = state
            .pairing
            .remaining_ms(&ticket.target.device_id, &ticket.generation)
        else {
            return Err(Cause::Stopped);
        };
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
        if let Message::PairVerification(verification) = message {
            match state
                .pairing_runtime
                .confirmations
                .process(state, verification)
                .map_err(|e| Cause::Failed(e.to_string()))?
            {
                plain_rs::services::discovery::pairing_confirmation::Action::Send {
                    ticket: reply_ticket,
                    message,
                } => {
                    if reply_ticket.generation != ticket.generation {
                        return Err(Cause::Stopped);
                    }
                    let mut send = facts.clone();
                    send["body"] = json!(message.wire().map_err(|e| Cause::Failed(e.to_string()))?);
                    if host(state, ticket, "blePairSend", send).await? != Value::Bool(true) {
                        return Err(Cause::Failed("Failed to send pairing verification".into()));
                    }
                }
                _ => return Err(Cause::Failed("Unexpected pairing verification step".into())),
            }
            continue;
        }
        let Message::PairResponse(response) = message else {
            continue;
        };
        if response.from_id != ticket.target.device_id {
            continue;
        }
        let _commit = state.pairing_runtime.commit.lock().unwrap();
        if state
            .pairing_runtime
            .attempts
            .get(&ticket.generation)
            .is_some_and(|s| s.terminal())
        {
            return Err(Cause::Stopped);
        }
        state
            .pairing_runtime
            .confirmations
            .response(&response)
            .map_err(|e| Cause::Failed(e.to_string()))?;
        let result = pairing::complete_checked(
            &state.db,
            &state.prefs,
            &state.pairing,
            response,
            "",
            Some(&ticket.generation),
        )
        .map_err(|e| Cause::Failed(e.to_string()))?;
        if result.is_null() {
            continue;
        }
        if result["peer"].is_null() {
            failed(
                state,
                &ticket.target,
                result["error"].as_str().unwrap_or("Pairing failed"),
                Some(&ticket.generation),
            );
        } else {
            state.pairing_runtime.attempts.transition(
                state,
                &ticket.generation,
                "SUCCEEDED",
                None,
                None,
            );
            success(state, &result["peer"], "");
        }
        return Ok(());
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/services/discovery/ble_pairing.rs"]
mod tests;
