use crate::infrastructure::transport::contract::PeerDispatch;
use crate::infrastructure::db::DPeer;
use crate::infrastructure::transport::router::Attempt;
use crate::infrastructure::transport::router::Outcome;
use crate::infrastructure::transport::router::Router;
use crate::infrastructure::transport::router::Ticket;
use crate::infrastructure::transport::router::TransportType;
use crate::runtime::Runtime as CoreRuntime;
use plain_libs::serde_json::{Value, json};
use std::{borrow::Cow, future::Future, time::Duration};
use tokio::{sync::Semaphore, time::Instant};
pub async fn capabilities(
    host: &crate::infrastructure::host::bridge::Host,
) -> Result<Vec<TransportType>, String> {
    if !host.connected() {
        return Ok(Vec::new());
    }
    plain_libs::serde_json::from_value(host.call("peerTransportCapabilities", json!({})).await?)
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
pub(crate) struct ChatLanes {
    routes: Semaphore,
    lan: Semaphore,
    ble: Semaphore,
}

impl Default for ChatLanes {
    fn default() -> Self {
        Self {
            routes: Semaphore::new(32),
            lan: Semaphore::new(3),
            ble: Semaphore::new(1),
        }
    }
}

impl ChatLanes {
    fn lane(&self, transport: TransportType) -> Option<&Semaphore> {
        match transport {
            TransportType::Lan => Some(&self.lan),
            TransportType::Ble => Some(&self.ble),
            TransportType::Aware => None,
        }
    }
}

async fn spend<T>(
    budget: &mut Option<Duration>,
    work: impl Future<Output = T>,
) -> Result<T, String> {
    let Some(left) = budget else {
        return Ok(work.await);
    };
    if left.is_zero() {
        return Err("Peer delivery unconfirmed after 20s".into());
    }
    let started = Instant::now();
    let result = tokio::time::timeout(*left, work)
        .await
        .map_err(|_| "Peer delivery unconfirmed after 20s".to_string())?;
    *left = left.saturating_sub(started.elapsed());
    Ok(result)
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
    let client_id = state.native_client_id()?;
    let signing_key = crate::features::prefs::security::signing_keypair(&state.prefs)
        .map_err(|error| error.to_string())?;
    let dispatch = PeerDispatch { peer, client_id: &client_id, channel_id, key, signing_key: &signing_key, message_id: None };
    send_checked_with_attempt(
        state,
        &dispatch,
        || Ok(Cow::Borrowed(body)),
        Attempt::Automatic,
        None,
        validate,
    )
    .await
}
pub(crate) async fn send_chat(
    state: &CoreRuntime,
    dispatch: &PeerDispatch<'_>,
    sign: impl Fn() -> Result<String, String>,
    attempt: Attempt,
) -> Result<Value, String> {
    send_checked_with_attempt(
        state,
        dispatch,
        || sign().map(Cow::Owned),
        attempt,
        Some(&state.chat_lanes),
        || Ok(()),
    )
    .await
}

async fn send_checked_with_attempt<'a>(
    state: &CoreRuntime,
    dispatch: &PeerDispatch<'_>,
    body: impl Fn() -> Result<Cow<'a, str>, String>,
    attempt: Attempt,
    lanes: Option<&ChatLanes>,
    validate: impl Fn() -> Result<(), String>,
) -> Result<Value, String> {
    let peer = dispatch.peer;
    if dispatch.key.len() != 32 {
        return Err("Invalid peer transport key".into());
    }
    if *state.stop.borrow() {
        return Err("Core stopped".into());
    }
    let mut stop = state.stop.clone();
    let _chat_route = match lanes {
        Some(lanes) => Some(tokio::select! {
            _ = stop.changed() => return Err("Core stopped".into()),
            permit = lanes.routes.acquire() => permit.map_err(|e| e.to_string())?,
        }),
        None => None,
    };
    let mut budget =
        lanes.map(|_| crate::infrastructure::transport::contract::CHAT_DELIVERY_TIMEOUT);
    let host = &state.host;
    let router = &state.transport;
    let mut available = vec![TransportType::Lan];
    available.extend(
        spend(&mut budget, capabilities(host))
            .await??
            .into_iter()
            .filter(|kind| *kind != TransportType::Lan),
    );
    validate()?;
    let current_peer =
        crate::features::services::peers::peer_address::current_identity(&state.db, &peer.id, peer)
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
        let _lane = match lanes.and_then(|lanes| lanes.lane(pending.ticket.transport)) {
            Some(lane) => Some(tokio::select! {
                _ = stop.changed() => return Err("Core stopped".into()),
                permit = lane.acquire() => permit.map_err(|e| e.to_string())?,
            }),
            None => None,
        };
        notify(state);
        validate()?;
        crate::features::services::peers::peer_address::current_identity(&state.db, &peer.id, peer)
            .map_err(|e| e.to_string())?;
        let body = body()?;
        if pending.ticket.transport == TransportType::Lan {
            let response = tokio::select! {
                _ = stop.changed() => return Err("Core stopped".into()),
                result = spend(&mut budget, crate::features::services::peers::peer_lan::send(state, dispatch, &body, &validate)) => result?,
            };
            match response {
                Ok(response) => {
                    validate()?;
                    crate::features::services::peers::peer_address::current_identity(
                        &state.db, &peer.id, peer,
                    )
                    .map_err(|e| e.to_string())?;
                    router
                        .finish(&pending.ticket, Outcome::Connected)
                        .map_err(|e| e.to_string())?;
                    return Ok(response);
                }
                Err(crate::features::services::peers::peer_lan::Failure::Fatal(error)) => {
                    return Err(error);
                }
                Err(crate::features::services::peers::peer_lan::Failure::Unavailable(error)) => {
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
        let exchange = tokio::time::timeout(Duration::from_secs(15), async {
            match pending.ticket.transport {
                TransportType::Aware => {
                    crate::features::services::peers::peer_sdk::aware_send(
                        state, dispatch, &body,
                    )
                    .await
                }
                TransportType::Ble => {
                    crate::features::services::peers::peer_sdk::ble_send(
                        state, dispatch, &body,
                    )
                    .await
                }
                TransportType::Lan => unreachable!(),
            }
        });
        let attempt = tokio::select! {
            _ = stop.changed() => return Err("Core stopped".into()),
            result = spend(&mut budget, exchange) => result?,
        };
        let outcome = match attempt {
            Err(_) => return Err("SDK response timed out; delivery is unconfirmed".into()),
            Ok(Ok(response)) => {
                validate()?;
                crate::features::services::peers::peer_address::current_identity(
                    &state.db, &peer.id, peer,
                )
                .map_err(|e| e.to_string())?;
                router
                    .finish(&pending.ticket, Outcome::Connected)
                    .map_err(|e| e.to_string())?;
                return Ok(response);
            }
            Ok(Err(crate::features::services::peers::peer_lan::Failure::Fatal(error))) => {
                return Err(error);
            }
            Ok(Err(crate::features::services::peers::peer_lan::Failure::Unavailable(error))) => {
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
) -> plain_libs::anyhow::Result<AwareConnectionConfig> {
    let key = if peer.key.is_empty() {
        Vec::new()
    } else {
        plain_libs::utils::base64::base64_decode_checked(&peer.key)?
    };
    plain_libs::anyhow::ensure!(
        key.is_empty() || key.len() == 32,
        "Invalid Aware pairing key"
    );
    let actor = state
        .prefs
        .get::<String>(crate::features::prefs::pref_keys::CLIENT_ID)?
        .filter(|s| !s.is_empty())
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Missing local identity"))?;
    Ok(AwareConnectionConfig {
        pmk: if key.is_empty() {
            None
        } else {
            Some(plain_libs::utils::base64::base64_encode(&key))
        },
        is_client: actor < peer.id,
        local_port: state.prefs.get::<u16>("https_port")?.unwrap_or(8443),
    })
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/peers/peer_transport.rs"]
mod tests;
