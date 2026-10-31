use plain_rs::db::DPeer;
use plain_rs::runtime::Runtime as CoreRuntime;
use plain_rs::transport::router::Router;
use plain_rs::transport::router::Ticket;
use plain_rs::transport::router::TransportType;
use plain_rs::transport::router::{Attempt, Outcome};
use serde_json::{Value, json};
use std::time::Duration;
pub async fn capabilities(
    host: &plain_rs::host::bridge::Host,
) -> Result<Vec<TransportType>, String> {
    if !host.connected() {
        return Ok(Vec::new());
    }
    serde_json::from_value(host.call("peerTransportCapabilities", json!({})).await?)
        .map_err(|error| error.to_string())
}
pub fn snapshot(state: &CoreRuntime) -> Value {
    let mut active: Vec<_> = state.transport.active().into_iter().collect();
    active.sort_by(|left, right| left.0.cmp(&right.0));
    json!({"active": active.into_iter().map(|(peer_id, transport)| {
        json!({"peerId": peer_id, "transport": transport})
    }).collect::<Vec<_>>()})
}

pub fn notify_snapshot(state: &CoreRuntime) -> Result<(), String> {
    let _delivery = state.transport_delivery.lock().unwrap();
    state.host.notify("peerTransportsChanged", snapshot(state))
}

pub fn notify(state: &CoreRuntime) {
    let mut recovering = state.transport_delivery.lock().unwrap();
    if let Err(error) = state.host.notify("peerTransportsChanged", snapshot(state)) {
        log::warn!("Peer transport callback: {error}");
        if *recovering || !state.host.connected() {
            return;
        }
        *recovering = true;
        let state = state.clone();
        tokio::spawn(async move {
            let permit = state.host.reserve_callback().await;
            let mut recovering = state.transport_delivery.lock().unwrap();
            if let Ok(permit) = permit {
                if let Err(error) = state.host.notify_reserved_callback(
                    "peerTransportsChanged",
                    snapshot(&state),
                    permit,
                ) {
                    log::warn!("Peer transport recovery callback: {error}");
                }
            }
            *recovering = false;
        });
    }
}
pub struct Pending<'a> {
    pub router: &'a Router,
    pub state: &'a CoreRuntime,
    pub ticket: Ticket,
}
impl Drop for Pending<'_> {
    fn drop(&mut self) {
        self.router.abort(&self.ticket);
        notify(self.state);
    }
}
pub async fn send(
    state: &CoreRuntime,
    peer: &DPeer,
    channel_id: &str,
    key: &[u8],
    body: &str,
) -> Result<Value, String> {
    send_checked(state, peer, channel_id, key, body, || Ok(())).await
}
pub async fn send_checked(
    state: &CoreRuntime,
    peer: &DPeer,
    channel_id: &str,
    key: &[u8],
    body: &str,
    validate: impl Fn() -> Result<(), String>,
) -> Result<Value, String> {
    send_checked_with_attempt(
        state,
        peer,
        channel_id,
        key,
        body,
        Attempt::Automatic,
        validate,
    )
    .await
}
pub async fn send_with_attempt(
    state: &CoreRuntime,
    peer: &DPeer,
    channel_id: &str,
    key: &[u8],
    body: &str,
    attempt: Attempt,
) -> Result<Value, String> {
    send_checked_with_attempt(state, peer, channel_id, key, body, attempt, || Ok(())).await
}
async fn send_checked_with_attempt(
    state: &CoreRuntime,
    peer: &DPeer,
    channel_id: &str,
    key: &[u8],
    body: &str,
    attempt: Attempt,
    validate: impl Fn() -> Result<(), String>,
) -> Result<Value, String> {
    if key.len() != 32 {
        return Err("Invalid peer transport key".into());
    }
    if *state.stop.borrow() {
        return Err("Core stopped".into());
    }
    let host = &state.host;
    let router = &state.transport;
    let mut available = vec![TransportType::Lan];
    available.extend(
        capabilities(host)
            .await?
            .into_iter()
            .filter(|kind| *kind != TransportType::Lan),
    );
    validate()?;
    let current_peer =
        plain_rs::services::peers::peer_address::current_identity(&state.db, &peer.id, peer)
            .map_err(|e| e.to_string())?;
    let mut step = router
        .begin_with_attempt(&current_peer, &available, attempt)
        .map_err(|e| e.to_string())?;
    let first = step.ticket.take().ok_or_else(|| {
        step.error
            .unwrap_or_else(|| "Peer transport unavailable".into())
    })?;
    let mut pending = Pending {
        state,
        router,
        ticket: first,
    };
    loop {
        notify(state);
        validate()?;
        plain_rs::services::peers::peer_address::current_identity(&state.db, &peer.id, peer)
            .map_err(|e| e.to_string())?;
        if pending.ticket.transport == TransportType::Lan {
            let mut stop = state.stop.clone();
            let response = tokio::select! {_=stop.changed()=>return Err("Core stopped".into()),result=plain_rs::services::peers::peer_lan::send(state,peer,channel_id,key,body,&validate)=>result};
            match response {
                Ok(response) => {
                    validate()?;
                    plain_rs::services::peers::peer_address::current_identity(
                        &state.db, &peer.id, peer,
                    )
                    .map_err(|e| e.to_string())?;
                    router
                        .finish(&pending.ticket, Outcome::Connected)
                        .map_err(|e| e.to_string())?;
                    return Ok(response);
                }
                Err(plain_rs::services::peers::peer_lan::Failure::Fatal(error)) => {
                    return Err(error);
                }
                Err(plain_rs::services::peers::peer_lan::Failure::Unavailable(error)) => {
                    let step = router
                        .finish(&pending.ticket, Outcome::Unavailable { error })
                        .map_err(|e| e.to_string())?;
                    match step.ticket {
                        Some(ticket) => {
                            pending.ticket = ticket;
                            continue;
                        }
                        None => {
                            return Err(step
                                .error
                                .unwrap_or_else(|| "Peer transport unavailable".into()));
                        }
                    }
                }
            }
        }
        let attempt = tokio::time::timeout(Duration::from_secs(15), async {
            match pending.ticket.transport {
                TransportType::Aware => {
                    plain_rs::services::peers::peer_sdk::aware_send(
                        state, peer, channel_id, key, body,
                    )
                    .await
                }
                TransportType::Ble => {
                    plain_rs::services::peers::peer_sdk::ble_send(
                        state, peer, channel_id, key, body,
                    )
                    .await
                }
                TransportType::Lan => unreachable!(),
            }
        })
        .await;
        let outcome = match attempt {
            Err(_) => return Err("SDK response timed out; delivery is unconfirmed".into()),
            Ok(Ok(response)) => {
                validate()?;
                plain_rs::services::peers::peer_address::current_identity(
                    &state.db, &peer.id, peer,
                )
                .map_err(|e| e.to_string())?;
                router
                    .finish(&pending.ticket, Outcome::Connected)
                    .map_err(|e| e.to_string())?;
                return Ok(response);
            }
            Ok(Err(plain_rs::services::peers::peer_lan::Failure::Fatal(error))) => {
                return Err(error);
            }
            Ok(Err(plain_rs::services::peers::peer_lan::Failure::Unavailable(error))) => {
                Outcome::Unavailable { error }
            }
        };
        let step = router
            .finish(&pending.ticket, outcome)
            .map_err(|e| e.to_string())?;
        match step.ticket {
            Some(ticket) => pending.ticket = ticket,
            None => {
                return Err(step
                    .error
                    .unwrap_or_else(|| "Peer transport unavailable".into()));
            }
        }
    }
}
#[derive(Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AwareConnectionConfig {
    pub pmk: Option<String>,
    pub is_client: bool,
    pub local_port: u16,
}
pub fn aware_config_for_peer(
    state: &CoreRuntime,
    peer: &DPeer,
) -> anyhow::Result<AwareConnectionConfig> {
    let key = if peer.key.is_empty() {
        Vec::new()
    } else {
        plain_rs::utils::base64::base64_decode_checked(&peer.key)?
    };
    anyhow::ensure!(
        key.is_empty() || key.len() == 32,
        "Invalid Aware pairing key"
    );
    let actor = state
        .prefs
        .get::<String>(plain_rs::prefs::pref_keys::CLIENT_ID)?
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("Missing local identity"))?;
    Ok(AwareConnectionConfig {
        pmk: if key.is_empty() {
            None
        } else {
            Some(plain_rs::utils::base64::base64_encode(&key))
        },
        is_client: actor < peer.id,
        local_port: state.prefs.get::<u16>("https_port")?.unwrap_or(8443),
    })
}

#[cfg(test)]
#[path = "../../../tests/unit/services/peers/peer_transport.rs"]
mod tests;
