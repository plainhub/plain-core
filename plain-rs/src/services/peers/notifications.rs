use anyhow::{Result, ensure};
use plain_rs::runtime::Runtime as CoreRuntime;
use plain_rs::services::notifications::types::Notification;
use plain_rs::services::peers::management::Identity;
use plain_rs::services::peers::management::current_peer;
use plain_rs::services::peers::metadata::AppSnapshot;
use plain_rs::{chat::enums::DeviceType, db::DPeer};
use serde::Serialize;
use serde_json::json;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{Arc, Mutex},
};
use tokio::sync::watch;

const LIMIT: usize = 200;

#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    #[serde(flatten)]
    notification: Notification,
    icon: String,
}

#[derive(Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct Group {
    peer_id: String,
    name: String,
    device_type: DeviceType,
    online: bool,
    loading: bool,
    loaded: bool,
    capabilities: Option<Vec<String>>,
    permissions: Vec<String>,
    items: Vec<Item>,
}

struct Peer {
    identity: Identity,
    group: Group,
    baseline: bool,
    url_token: String,
    icons: HashMap<String, String>,
}

#[derive(Default)]
struct State {
    revision: u64,
    peers: BTreeMap<String, Peer>,
    recovery_pending: bool,
    missed_alerts: Vec<Item>,
    missed_permissions_changed: bool,
}

#[derive(Default)]
pub struct Store(Arc<Mutex<State>>);

fn allowed(app: &AppSnapshot) -> bool {
    app.capabilities
        .iter()
        .any(|value| value == "NOTIFICATIONS")
        && app
            .permissions
            .iter()
            .any(|value| value == "NOTIFICATION_LISTENER")
}

impl State {
    fn snapshot(&self) -> serde_json::Value {
        let groups: Vec<_> = self.peers.values().map(|peer| &peer.group).collect();
        json!({"revision":self.revision,"groups":groups})
    }
}

impl Store {
    fn publish(
        &self,
        state: &mut State,
        id: &str,
        alerts: Vec<Item>,
        permissions_changed: bool,
        host: &Arc<plain_rs::host::bridge::Host>,
    ) {
        state.revision += 1;
        if state.recovery_pending {
            state.missed_alerts.extend(alerts);
            state.missed_permissions_changed |= permissions_changed;
            return;
        }
        if !host.connected() {
            return;
        }
        let group = state.peers.get(id).map(|peer| &peer.group);
        let params = json!({"revision":state.revision,"peerId":id,"group":group,"alerts":alerts,"permissionsChanged":permissions_changed});
        if let Err(error) = host.notify("peerNotificationsChanged", params) {
            log::warn!("Peer notifications callback: {error}");
            state.missed_alerts.extend(alerts);
            state.missed_permissions_changed |= permissions_changed;
            state.recovery_pending = true;
            let shared = self.0.clone();
            let host = host.clone();
            tokio::spawn(async move {
                let permit = host.reserve_callback().await;
                let mut state = shared.lock().unwrap();
                if let Ok(permit) = permit {
                    let mut snapshot = state.snapshot();
                    snapshot["alerts"] = json!(state.missed_alerts);
                    snapshot["permissionsChanged"] = json!(state.missed_permissions_changed);
                    match host.notify_reserved_callback(
                        "peerNotificationsChanged",
                        snapshot,
                        permit,
                    ) {
                        Ok(()) => {
                            state.missed_alerts.clear();
                            state.missed_permissions_changed = false;
                        }
                        Err(error) => log::warn!("Peer notifications snapshot callback: {error}"),
                    }
                }
                state.recovery_pending = false;
            });
        }
    }

    pub fn reconcile(
        &self,
        peers: &[DPeer],
        wanted: &HashMap<String, Identity>,
        host: &Arc<plain_rs::host::bridge::Host>,
    ) {
        let mut state = self.0.lock().unwrap();
        let removed: Vec<_> = state
            .peers
            .keys()
            .filter(|id| !wanted.contains_key(*id))
            .cloned()
            .collect();
        for id in removed {
            state.peers.remove(&id);
            self.publish(&mut state, &id, vec![], false, host);
        }
        for peer in peers {
            let Some(identity) = wanted.get(&peer.id) else {
                continue;
            };
            if state
                .peers
                .get(&peer.id)
                .is_none_or(|old| old.identity.token != identity.token)
            {
                state.peers.insert(
                    peer.id.clone(),
                    Peer {
                        identity: identity.clone(),
                        group: Group {
                            peer_id: peer.id.clone(),
                            name: peer.name.clone(),
                            device_type: peer.device_type,
                            online: false,
                            loading: true,
                            loaded: false,
                            capabilities: None,
                            permissions: vec![],
                            items: vec![],
                        },
                        baseline: false,
                        url_token: String::new(),
                        icons: HashMap::new(),
                    },
                );
                self.publish(&mut state, &peer.id, vec![], false, host);
            } else if let Some(old) = state.peers.get_mut(&peer.id) {
                if old.identity != *identity {
                    old.identity = identity.clone();
                    old.icons.clear();
                }
                if old.group.name != peer.name || old.group.device_type != peer.device_type {
                    old.group.name = peer.name.clone();
                    old.group.device_type = peer.device_type;
                    self.publish(&mut state, &peer.id, vec![], false, host);
                }
            }
        }
    }

    pub fn snapshot(&self) -> serde_json::Value {
        self.0.lock().unwrap().snapshot()
    }

    pub fn notify_snapshot(&self, host: &plain_rs::host::bridge::Host) -> Result<(), String> {
        let mut state = self.0.lock().unwrap();
        let mut snapshot = state.snapshot();
        if !state.missed_alerts.is_empty() {
            snapshot["alerts"] = json!(state.missed_alerts);
        }
        if state.missed_permissions_changed {
            snapshot["permissionsChanged"] = json!(true);
        }
        host.notify("peerNotificationsChanged", snapshot)?;
        state.missed_alerts.clear();
        state.missed_permissions_changed = false;
        Ok(())
    }

    fn apply(
        &self,
        id: &str,
        identity: &Identity,
        result: Result<(AppSnapshot, Vec<Notification>, reqwest::Url)>,
        host: &Arc<plain_rs::host::bridge::Host>,
    ) -> Result<()> {
        let mut state = self.0.lock().unwrap();
        let Some(peer) = state
            .peers
            .get_mut(id)
            .filter(|peer| peer.identity == *identity)
        else {
            return Ok(());
        };
        let before = peer.group.clone();
        let mut alerts = vec![];
        match result {
            Ok((app, items, base)) => {
                validate_snapshot(&app, &items)?;
                let is_allowed = allowed(&app);
                if peer.url_token != app.url_token {
                    peer.icons.clear();
                    peer.url_token = app.url_token;
                }
                let mut projected = Vec::with_capacity(items.len());
                if is_allowed {
                    let previous: HashSet<_> = peer
                        .group
                        .items
                        .iter()
                        .map(|item| item.notification.id.as_str().to_owned())
                        .collect();
                    for notification in items {
                        let icon = match peer.icons.get(notification.app_id.as_str()) {
                            Some(icon) => icon.clone(),
                            None => {
                                let file_id = plain_rs::chat::messages::content::make_file_id(
                                    &format!("pkgicon://{}", notification.app_id.as_str()),
                                    &peer.url_token,
                                );
                                ensure!(!file_id.is_empty(), "Invalid notification icon key");
                                let mut url = base.join("/fs")?;
                                url.query_pairs_mut().append_pair("id", &file_id);
                                let icon = url.to_string();
                                peer.icons
                                    .insert(notification.app_id.to_string(), icon.clone());
                                icon
                            }
                        };
                        let fresh = !previous.contains(notification.id.as_str());
                        let item = Item { notification, icon };
                        if peer.baseline && fresh && !item.notification.silent {
                            alerts.push(item.clone());
                        }
                        projected.push(item);
                    }
                }
                peer.baseline = is_allowed;
                peer.group.items = projected;
                peer.group.capabilities = Some(app.capabilities);
                peer.group.permissions = app.permissions;
                peer.group.online = true;
            }
            Err(error)
                if matches!(
                    error.downcast_ref::<plain_rs::services::peers::management_http::AuthorizationError>(),
                    Some(plain_rs::services::peers::management_http::AuthorizationError::Permission)
                ) =>
            {
                peer.group.items.clear();
                peer.group
                    .permissions
                    .retain(|name| name != "NOTIFICATION_LISTENER");
                peer.baseline = false;
                peer.group.online = true;
            }
            Err(error)
                if matches!(
                    error.downcast_ref::<plain_rs::services::peers::management_http::AuthorizationError>(),
                    Some(plain_rs::services::peers::management_http::AuthorizationError::Session)
                ) =>
            {
                peer.group.items.clear();
                peer.baseline = false;
                peer.group.online = false;
            }
            Err(error) => {
                log::debug!("Peer notification refresh failed: {error}");
                peer.group.online = false;
            }
        }
        peer.group.loading = false;
        peer.group.loaded = true;
        if peer.group != before || !alerts.is_empty() {
            let permissions_changed = peer.group.permissions != before.permissions;
            self.publish(&mut state, id, alerts, permissions_changed, host);
        }
        Ok(())
    }
}

fn validate_snapshot(app: &AppSnapshot, items: &[Notification]) -> Result<()> {
    ensure!(items.len() <= LIMIT, "Invalid notification snapshot");
    let mut ids = HashSet::new();
    ensure!(
        items
            .iter()
            .all(|item| !item.id.as_str().is_empty() && ids.insert(item.id.as_str())),
        "Invalid notification identities"
    );
    ensure!(
        !allowed(app)
            || items.is_empty()
            || plain_rs::utils::base64::base64_decode(&app.url_token).len() == 32,
        "Invalid notification icon key"
    );
    Ok(())
}

pub async fn refresh_from_app(
    state: &CoreRuntime,
    id: &str,
    identity: &Identity,
    cancelled: &watch::Receiver<bool>,
    app: Result<AppSnapshot>,
) -> Result<()> {
    if !current_peer(state, id, identity, cancelled).await? {
        return Ok(());
    }
    let base = identity.http_url()?;
    let client_id = state.native_client_id().map_err(anyhow::Error::msg)?;
    let key = plain_rs::utils::base64::base64_decode(&identity.token);
    let result = async {
        let app = app?;
        let items: Vec<Notification> = if allowed(&app) {
            let data = plain_rs::services::peers::management_http::query(state, &base, &client_id, &key,
                json!({"query":"{ notifications(query:\"\",offset:0,limit:200) { id onlyOnce isClearable appId appName postedAt silent title body actions replyActions } }"}), 4 * 1024 * 1024).await?;
            serde_json::from_value(data["notifications"].clone())?
        } else { vec![] };
        validate_snapshot(&app, &items)?;
        Ok((app, items, base))
    }.await;
    let state = state.clone();
    let id = id.to_owned();
    let identity = identity.clone();
    let cancelled = cancelled.clone();
    tokio::task::spawn_blocking(move || {
        state.db.with_conn(|connection| {
            if plain_rs::services::peers::management::current(
                connection, &state, &id, &identity, &cancelled,
            )? {
                state
                    .management
                    .notifications
                    .apply(&id, &identity, result, &state.host)?;
            }
            Ok(())
        })
    })
    .await?
}

#[cfg(test)]
#[path = "../../../tests/unit/services/peers/notifications.rs"]
mod tests;
