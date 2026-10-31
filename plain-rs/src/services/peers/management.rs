use anyhow::{Result, anyhow, ensure};
use futures_util::{SinkExt, StreamExt};
use plain_rs::db::{DPeer, chat_store::peers};
use plain_rs::runtime::Runtime as CoreRuntime;
use rusqlite::OptionalExtension;
use std::{collections::HashMap, sync::Mutex, time::Duration};
use tokio::{sync::watch, task::JoinHandle};
use tokio_tungstenite::tungstenite::Message;

#[derive(Clone, PartialEq, Eq)]
pub struct Identity {
    pub url: String,
    pub token: String,
    pub ip: String,
    pub port: Option<u16>,
}

impl Identity {
    pub fn http_url(&self) -> Result<reqwest::Url> {
        let mut url = reqwest::Url::parse(&self.url)?;
        url.set_scheme("https")
            .map_err(|_| anyhow!("Invalid management address"))?;
        url.set_query(None);
        Ok(url)
    }
}

struct Worker {
    identity: Identity,
    cancel: watch::Sender<bool>,
    job: JoinHandle<()>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel.send_replace(true);
        self.job.abort();
    }
}

#[derive(Default)]
pub struct Runtime {
    coordinator: Mutex<Option<(watch::Sender<bool>, JoinHandle<()>)>>,
    pub notifications: plain_rs::services::peers::notifications::Store,
    clipboard_delivery: Mutex<bool>,
}

impl Runtime {
    pub fn notify_clipboard_snapshot(
        &self,
        host: &plain_rs::host::bridge::Host,
    ) -> Result<(), String> {
        let _delivery = self.clipboard_delivery.lock().unwrap();
        host.notify(
            "peerClipboardChanged",
            serde_json::json!({"peerIds":[],"refreshAll":true}),
        )
    }

    fn notify_clipboard(state: &CoreRuntime, peer_id: &str) {
        let mut recovering = state.management.clipboard_delivery.lock().unwrap();
        if let Err(error) = state.host.notify(
            "peerClipboardChanged",
            serde_json::json!({"peerIds":[peer_id],"refreshAll":false}),
        ) {
            log::warn!("Peer clipboard callback: {error}");
            if *recovering || !state.host.connected() {
                return;
            }
            *recovering = true;
            let state = state.clone();
            tokio::spawn(async move {
                let permit = state.host.reserve_callback().await;
                let mut recovering = state.management.clipboard_delivery.lock().unwrap();
                if let Ok(permit) = permit {
                    if let Err(error) = state.host.notify_reserved_callback(
                        "peerClipboardChanged",
                        serde_json::json!({"peerIds":[],"refreshAll":true}),
                        permit,
                    ) {
                        log::warn!("Peer clipboard recovery callback: {error}");
                    }
                }
                *recovering = false;
            });
        }
    }
    pub fn start(&self, state: CoreRuntime) -> Result<()> {
        let actor = state.native_client_id().map_err(anyhow::Error::msg)?;
        let mut slot = self
            .coordinator
            .lock()
            .map_err(|_| anyhow!("Management runtime unavailable"))?;
        ensure!(slot.is_none(), "Management runtime already started");
        let mut changed = state.db.peer_changes();
        let mut network = state.network_revision.subscribe();
        let (cancel, mut cancelled) = watch::channel(false);
        let coordinator = tokio::spawn(async move {
            let mut stop = state.stop.clone();
            let mut workers = HashMap::new();
            loop {
                if *stop.borrow() || *cancelled.borrow() {
                    break;
                }
                let wanted = tokio::select! {
                    biased;
                    _ = stop.changed() => break,
                    _ = cancelled.changed() => break,
                    wanted = desired(&state, &actor) => wanted,
                };
                let retry = match wanted {
                    Ok((peers, wanted)) => {
                        state
                            .management
                            .notifications
                            .reconcile(&peers, &wanted, &state.host);
                        reconcile(&state, &mut workers, wanted).await;
                        false
                    }
                    Err(error) => {
                        log::warn!("Management peer reconciliation: {error}");
                        true
                    }
                };
                let retry_delay = tokio::time::sleep(Duration::from_secs(5));
                tokio::pin!(retry_delay);
                let again = loop {
                    tokio::select! {
                        biased;
                        _ = stop.changed() => break false,
                        _ = cancelled.changed() => break false,
                        result = changed.changed() => break result.is_ok(),
                        _ = &mut retry_delay, if retry => break true,
                        result = network.changed() => break result.is_ok(),
                    }
                };
                if !again {
                    break;
                }
            }
            retire(&mut workers).await;
        });
        *slot = Some((cancel, coordinator));
        Ok(())
    }

    pub async fn shutdown(&self) {
        let coordinator = self.coordinator.lock().unwrap().take();
        if let Some((cancel, coordinator)) = coordinator {
            cancel.send_replace(true);
            let _ = coordinator.await;
        }
    }
}

pub fn identity(peer: &DPeer, actor: &str, base: &str) -> Option<Identity> {
    if peer.id == actor
        || peer.port.is_none_or(|port| port == 0)
        || actor.is_empty()
        || plain_rs::utils::base64::base64_decode(&peer.token).len() != 32
    {
        return None;
    }
    let mut url = reqwest::Url::parse(base).ok()?;
    if url.scheme() != "https" || url.host_str().is_none() {
        return None;
    }
    url.set_scheme("wss").ok()?;
    url.set_path("/");
    url.set_query(None);
    url.set_fragment(None);
    url.query_pairs_mut().append_pair("cid", actor);
    Some(Identity {
        url: url.to_string(),
        token: peer.token.clone(),
        ip: peer.ip.clone(),
        port: peer.port,
    })
}

async fn desired(
    state: &CoreRuntime,
    actor: &str,
) -> Result<(Vec<DPeer>, HashMap<String, Identity>)> {
    let state = state.clone();
    let actor = actor.to_owned();
    tokio::task::spawn_blocking(move || -> Result<(Vec<DPeer>, HashMap<String, Identity>)> {
        let interfaces = plain_rs::transport::lan::local_interfaces();
        let peers = peers::all(&state.db)?;
        let wanted = peers
            .iter()
            .filter_map(|peer| {
                let address = plain_rs::services::peers::peer_address::address_with_interfaces(
                    peer,
                    &interfaces,
                );
                identity(peer, &actor, &address.base_url)
                    .map(|identity| (peer.id.clone(), identity))
            })
            .collect();
        Ok((peers, wanted))
    })
    .await?
}

async fn reconcile(
    state: &CoreRuntime,
    workers: &mut HashMap<String, Worker>,
    wanted: HashMap<String, Identity>,
) {
    let removed: Vec<_> = workers
        .iter()
        .filter(|(id, worker)| {
            worker.job.is_finished()
                || wanted
                    .get(*id)
                    .is_none_or(|identity| identity != &worker.identity)
        })
        .map(|(id, _)| id.clone())
        .collect();
    for id in removed {
        if let Some(mut worker) = workers.remove(&id) {
            worker.cancel.send_replace(true);
            worker.job.abort();
            let _ = (&mut worker.job).await;
        }
    }
    for (id, identity) in wanted {
        workers.entry(id.clone()).or_insert_with(|| {
            let (cancel, cancelled) = watch::channel(false);
            let context = state.clone();
            let connection = identity.clone();
            let job = tokio::spawn(async move {
                run(context, id, connection, cancelled).await;
            });
            Worker {
                identity,
                cancel,
                job,
            }
        });
    }
}

async fn retire(workers: &mut HashMap<String, Worker>) {
    for (_, mut worker) in workers.drain() {
        worker.cancel.send_replace(true);
        worker.job.abort();
        let _ = (&mut worker.job).await;
    }
}

async fn publish_clipboard_change(
    state: &CoreRuntime,
    peer_id: &str,
    identity: &Identity,
    cancelled: &watch::Receiver<bool>,
) -> Result<bool> {
    let state = state.clone();
    let peer_id = peer_id.to_owned();
    let identity = identity.clone();
    let cancelled = cancelled.clone();
    tokio::task::spawn_blocking(move || {
        state.db.with_conn(|connection| -> Result<bool> {
            if !current(connection, &state, &peer_id, &identity, &cancelled)? {
                return Ok(false);
            }
            Runtime::notify_clipboard(&state, &peer_id);
            Ok(true)
        })
    })
    .await?
}

pub fn current(
    connection: &rusqlite::Connection,
    state: &CoreRuntime,
    peer_id: &str,
    identity: &Identity,
    cancelled: &watch::Receiver<bool>,
) -> Result<bool> {
    let current = connection
        .query_row(
            "SELECT token,ip,port FROM peers WHERE id=?1",
            [peer_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<u16>>(2)?,
                ))
            },
        )
        .optional()?;
    Ok(!*cancelled.borrow()
        && !*state.stop.borrow()
        && current.as_ref().is_some_and(|(token, ip, port)| {
            token == &identity.token && ip == &identity.ip && port == &identity.port
        }))
}

pub async fn current_peer(
    state: &CoreRuntime,
    peer_id: &str,
    identity: &Identity,
    cancelled: &watch::Receiver<bool>,
) -> Result<bool> {
    let (state, peer_id, identity, cancelled) = (
        state.clone(),
        peer_id.to_owned(),
        identity.clone(),
        cancelled.clone(),
    );
    tokio::task::spawn_blocking(move || {
        state
            .db
            .with_conn(|connection| current(connection, &state, &peer_id, &identity, &cancelled))
    })
    .await?
}

pub fn next_delay(delay: Duration) -> Duration {
    (delay * 2).min(Duration::from_secs(30))
}

pub async fn refresh_data(
    state: &CoreRuntime,
    id: &str,
    identity: &Identity,
    cancelled: &watch::Receiver<bool>,
    clipboard_access: Option<&watch::Sender<Option<bool>>>,
) -> Result<()> {
    let app =
        match plain_rs::services::peers::metadata::refresh(state, id, identity, cancelled).await {
            Ok(Some(app)) => Ok(app),
            Ok(None) => {
                if let Some(access) = clipboard_access {
                    access.send_replace(None);
                }
                return Ok(());
            }
            Err(error) => Err(error),
        };
    let clipboard = app.as_ref().ok().map(|app| {
        app.capabilities.iter().any(|value| value == "CLIPBOARD")
            && app.permissions.iter().any(|value| value == "CLIPBOARD")
    });
    if let Some(access) = clipboard_access {
        access.send_replace(clipboard);
    }
    plain_rs::services::peers::notifications::refresh_from_app(state, id, identity, cancelled, app)
        .await
}

async fn follow_data(
    state: &CoreRuntime,
    id: &str,
    identity: &Identity,
    cancelled: &watch::Receiver<bool>,
    changed: &tokio::sync::Notify,
    clipboard_access: watch::Sender<Option<bool>>,
) {
    let mut interval = tokio::time::interval_at(
        tokio::time::Instant::now() + Duration::from_secs(60),
        Duration::from_secs(60),
    );
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        if let Err(error) =
            refresh_data(state, id, identity, cancelled, Some(&clipboard_access)).await
        {
            log::debug!("Peer data refresh failed: {error}");
        }
        tokio::select! {
            _ = interval.tick() => {},
            _ = changed.notified() => {},
        }
    }
}

async fn run(
    state: CoreRuntime,
    id: String,
    identity: Identity,
    mut cancelled: watch::Receiver<bool>,
) {
    let changed = tokio::sync::Notify::new();
    let clipboard_changed = tokio::sync::Notify::new();
    let (clipboard_access, access) = watch::channel(None);
    let mut stop = state.stop.clone();
    let publication_cancelled = cancelled.clone();
    tokio::select! {
        biased;
        _ = stop.wait_for(|stopped| *stopped) => {},
        _ = cancelled.wait_for(|cancelled| *cancelled) => {},
        _ = follow_data(&state, &id, &identity, &publication_cancelled, &changed, clipboard_access) => {},
        _ = connection_loop(state.clone(), id.clone(), identity.clone(), publication_cancelled.clone(), &changed, &clipboard_changed) => {},
        _ = plain_rs::services::peers::clipboard::follow(&state, &id, &identity, &publication_cancelled, &clipboard_changed, access) => {},
    }
}

async fn connection_loop(
    state: CoreRuntime,
    id: String,
    identity: Identity,
    mut cancelled: watch::Receiver<bool>,
    data_changed: &tokio::sync::Notify,
    clipboard_changed: &tokio::sync::Notify,
) {
    let key = plain_rs::utils::base64::base64_decode(&identity.token);
    let mut stop = state.stop.clone();
    let mut delay = Duration::from_secs(1);
    loop {
        if *stop.borrow() || *cancelled.borrow() {
            break;
        }
        let connected = tokio::select! {
            biased;
            _ = stop.changed() => break,
            _ = cancelled.changed() => break,
            connected = tokio::time::timeout(Duration::from_secs(10), plain_rs::services::peers::status_socket::connect_management(&identity.url)) => connected,
        };
        if let Ok(Ok(mut socket)) = connected {
            let publication_cancelled = cancelled.clone();
            let exchange = async {
                use plain_rs::protocols::graphql::messages::ClientMessage;
                use plain_rs::services::peers::management_wire;
                use plain_rs::services::peers::management_wire::Incoming;
                let url = reqwest::Url::parse(&identity.url)?;
                let client = url
                    .query_pairs()
                    .find(|(name, _)| name == "cid")
                    .map(|(_, value)| value.into_owned())
                    .ok_or_else(|| anyhow!("Management client missing"))?;
                let registration = management_wire::registration(
                    &key,
                    &client,
                    chrono::Utc::now().timestamp_millis(),
                )?;
                socket
                    .send(Message::Binary(registration.clone().into()))
                    .await?;
                let ack = tokio::time::timeout(Duration::from_secs(10), socket.next())
                    .await?
                    .ok_or_else(|| anyhow!("Management handshake closed"))??;
                let Message::Binary(ack) = ack else {
                    return Err(anyhow!("Encrypted management acknowledgement required"));
                };
                let mut stream = management_wire::acknowledge(&key, &client, &registration, &ack)?;
                for spec in management_wire::operations() {
                    let message = ClientMessage::Subscribe {
                        id: spec.field.into(),
                        payload: async_graphql::Request::new(
                            plain_rs::protocols::graphql::events::document(spec),
                        ),
                    };
                    socket
                        .send(Message::Binary(
                            management_wire::encode(&message, &key, &mut stream)?.into(),
                        ))
                        .await?;
                }
                clipboard_changed.notify_one();
                let messages = async {
                    let mut heartbeat = tokio::time::interval_at(
                        tokio::time::Instant::now() + Duration::from_secs(20),
                        Duration::from_secs(20),
                    );
                    let mut pong_deadline = None;
                    loop {
                        let message = tokio::select! {
                            _ = heartbeat.tick() => {
                                socket.send(Message::Binary(management_wire::encode(&ClientMessage::Ping { payload: None }, &key, &mut stream)?.into())).await?;
                                pong_deadline = Some(tokio::time::Instant::now() + Duration::from_secs(10));
                                continue;
                            },
                            _ = async { if let Some(deadline) = pong_deadline { tokio::time::sleep_until(deadline).await } else { std::future::pending::<()>().await } } => return Err(anyhow!("Management heartbeat timeout")),
                            message = socket.next() => message,
                        };
                        let Some(message) = message else {
                            break;
                        };
                        match message? {
                            Message::Binary(frame) => {
                                match management_wire::event(&frame, &key, &mut stream)? {
                                    Incoming::Pong => pong_deadline = None,
                                    Incoming::Ready => {
                                        data_changed.notify_one();
                                        clipboard_changed.notify_one();
                                    }
                                    Incoming::Rejected(error) => {
                                        log::debug!("Management subscription rejected: {error}")
                                    }
                                    Incoming::Event(kind) => {
                                        if kind == "CLIPBOARD_CHANGED" {
                                            if !publish_clipboard_change(
                                                &state,
                                                &id,
                                                &identity,
                                                &publication_cancelled,
                                            )
                                            .await?
                                            {
                                                break;
                                            }
                                            clipboard_changed.notify_one();
                                        } else {
                                            data_changed.notify_one();
                                        }
                                        delay = Duration::from_secs(1);
                                        if kind == "PERMISSIONS_UPDATED" {
                                            break;
                                        }
                                    }
                                }
                            }
                            Message::Ping(bytes) => socket.send(Message::Pong(bytes)).await?,
                            Message::Close(_) => break,
                            Message::Pong(_) => {}
                            _ => return Err(anyhow!("Invalid management subscription frame")),
                        }
                    }
                    Ok::<_, anyhow::Error>(())
                };
                messages.await
            };
            tokio::select! {
                biased;
                _ = stop.changed() => break,
                _ = cancelled.changed() => break,
                result = exchange => if let Err(error) = result { log::debug!("Management peer connection ended: {error}"); },
            }
        }
        tokio::select! {
            biased;
            _ = stop.changed() => break,
            _ = cancelled.changed() => break,
            _ = tokio::time::sleep(delay) => {},
        }
        delay = next_delay(delay);
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/services/peers/management.rs"]
mod tests;
