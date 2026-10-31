use anyhow::{Result, ensure};
use plain_rs::chat::enums::PeerStatus;
use plain_rs::db::chat_store::peers;
use plain_rs::runtime::Runtime as CoreRuntime;
use plain_rs::services::peers::peer_transport::AwareConnectionConfig;
use plain_rs::services::peers::peer_transport::aware_config_for_peer;
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;

#[derive(Clone, PartialEq, Eq, Serialize)]
struct Peer {
    id: String,
    port: Option<u16>,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
struct Subscription {
    peer: Peer,
    config: AwareConnectionConfig,
}

#[derive(Default)]
struct Work {
    enabled: bool,
    requested: HashMap<String, PeerStatus>,
    applied: HashMap<String, (u64, Subscription)>,
    rejected: HashMap<String, (u64, Subscription)>,
}

#[derive(Default)]
pub struct Runtime {
    work: tokio::sync::Mutex<Work>,
}

impl Runtime {
    pub async fn ensure(&self, state: &CoreRuntime) -> Result<()> {
        plain_rs::services::capabilities::require(&state.host, "WIFI_AWARE").await?;
        let mut work = self.work.lock().await;
        work.enabled = true;
        work.rejected.clear();
        self.reconcile(state, &mut work).await
    }

    pub async fn subscribe(&self, state: &CoreRuntime, id: &str) -> Result<bool> {
        plain_rs::services::capabilities::require(&state.host, "WIFI_AWARE").await?;
        let mut work = self.work.lock().await;
        let peer = peers::get(&state.db, id)?.ok_or_else(|| anyhow::anyhow!("Unknown peer"))?;
        work.requested.insert(id.to_owned(), peer.status);
        work.rejected.remove(id);
        self.reconcile(state, &mut work).await?;
        Ok(work.applied.contains_key(id))
    }

    pub async fn stop(&self, state: &CoreRuntime) -> Result<()> {
        let mut work = self.work.lock().await;
        work.enabled = false;
        work.requested.clear();
        work.rejected.clear();
        self.reconcile(state, &mut work).await
    }

    pub async fn refresh(&self, state: &CoreRuntime) -> Result<()> {
        let mut work = self.work.lock().await;
        self.reconcile(state, &mut work).await
    }

    async fn permissions_changed(&self, state: &CoreRuntime) -> Result<()> {
        let mut work = self.work.lock().await;
        work.rejected.clear();
        self.reconcile(state, &mut work).await
    }

    async fn reconcile(&self, state: &CoreRuntime, work: &mut Work) -> Result<()> {
        let Some(generation) = state.host.generation() else {
            work.applied.clear();
            work.rejected.clear();
            return Ok(());
        };
        work.applied
            .retain(|_, (applied, _)| *applied == generation);
        work.rejected
            .retain(|_, (applied, _)| *applied == generation);
        if !work.enabled && work.requested.is_empty() && work.applied.is_empty() {
            return Ok(());
        }
        let rows: HashMap<_, _> = peers::all(&state.db)?
            .into_iter()
            .map(|peer| (peer.id.clone(), peer))
            .collect();
        work.requested
            .retain(|id, status| rows.get(id).is_some_and(|peer| peer.status == *status));
        let wanted: HashMap<_, _> = rows
            .into_iter()
            .filter(|(id, peer)| {
                (work.enabled && peer.is_paired()) || work.requested.contains_key(id)
            })
            .collect();
        work.rejected.retain(|id, _| wanted.contains_key(id));
        let removed: Vec<_> = work
            .applied
            .keys()
            .filter(|id| !wanted.contains_key(*id))
            .cloned()
            .collect();
        for id in removed {
            stop_peer(state, &id).await?;
            work.applied.remove(&id);
        }
        let mut configuration_error = None;
        for (id, peer) in wanted {
            let config = match aware_config_for_peer(state, &peer) {
                Ok(config) => config,
                Err(error) => {
                    if work.applied.contains_key(&id) {
                        stop_peer(state, &id).await?;
                        work.applied.remove(&id);
                    }
                    configuration_error = Some(error);
                    continue;
                }
            };
            let subscription = Subscription {
                peer: Peer {
                    id: id.clone(),
                    port: peer.port,
                },
                config,
            };
            if work
                .applied
                .get(&id)
                .or_else(|| work.rejected.get(&id))
                .is_some_and(|(_, previous)| *previous == subscription)
            {
                continue;
            }
            let started = state
                .host
                .call("peerStartAware", serde_json::to_value(&subscription)?)
                .await
                .map_err(anyhow::Error::msg)?;
            ensure!(state.host.is_current(generation), "Aware host replaced");
            if started == json!(true) {
                work.rejected.remove(&id);
                work.applied.insert(id, (generation, subscription));
            } else {
                work.applied.remove(&id);
                ensure!(started == json!(false), "Invalid Aware start response");
                work.rejected.insert(id, (generation, subscription));
            }
        }
        match configuration_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

async fn stop_peer(state: &CoreRuntime, id: &str) -> Result<()> {
    let stopped = state
        .host
        .call("peerStopAware", json!({"id":id}))
        .await
        .map_err(anyhow::Error::msg)?;
    ensure!(stopped == json!(true), "Invalid Aware stop response");
    Ok(())
}

pub fn start(state: CoreRuntime) {
    let mut peers = state.db.peer_changes();
    let mut connection = state.host.connection_changes();
    let mut events = state.events.subscribe();
    tokio::spawn(async move {
        let mut stop = state.stop.clone();
        if *stop.borrow() {
            return;
        }
        let mut retry = false;
        loop {
            let permissions_changed = tokio::select! {
                _ = stop.changed() => break,
                changed = peers.changed() => { if changed.is_err() { break; } false },
                changed = connection.changed() => { if changed.is_err() { break; } false },
                event = events.recv() => match event {
                    Ok(event) if event.event_type == "PERMISSIONS_UPDATED" => true,
                    Ok(event) if event.event_type == "PREFS_UPDATED" => false,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => true,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    _ => continue,
                },
                _ = tokio::time::sleep(std::time::Duration::from_secs(5)), if retry => false,
            };
            let result = tokio::select! {
                _ = stop.changed() => break,
                result = async {
                    if permissions_changed { state.aware.permissions_changed(&state).await }
                    else { state.aware.refresh(&state).await }
                } => result,
            };
            retry = result.is_err();
            if let Err(error) = result {
                log::warn!("Aware configuration refresh: {error}");
            }
        }
    });
}

#[cfg(test)]
#[path = "../../../tests/unit/services/peers/aware.rs"]
mod tests;
