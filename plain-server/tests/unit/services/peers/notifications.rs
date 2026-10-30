async fn refresh(
    state: &crate::assembly::server::ServerState,
    id: &str,
    identity: &super::super::management::Identity,
    cancelled: &tokio::sync::watch::Receiver<bool>,
) -> anyhow::Result<()> {
    super::super::management::refresh_data(state, id, identity, cancelled, None).await
}
use super::super::metadata::AppSnapshot as App;
use super::*;
use crate::graphql::types::Instant;
use std::time::Duration;
use tokio::sync::Notify;
use plain_rs::events::WsEvent;
use std::sync::Arc;

struct CallbackReceiver {
    host: Arc<crate::host::bridge::Host>,
    generation: u64,
    requests: tokio::sync::mpsc::Receiver<serde_json::Value>,
    buffered: std::collections::VecDeque<serde_json::Value>,
}

impl CallbackReceiver {
    fn new(host: Arc<crate::host::bridge::Host>) -> Self {
        let (generation, requests) = host.connect();
        Self { host, generation, requests, buffered: Default::default() }
    }

    fn try_recv(&mut self) -> Result<serde_json::Value, tokio::sync::mpsc::error::TryRecvError> {
        if let Some(value) = self.buffered.pop_front() { return Ok(value); }
        loop {
            let request = self.requests.try_recv()?;
            self.host.reply(self.generation, json!({"id":request["id"],"result":null})).unwrap();
            if request["method"] == "peerNotificationsChanged" {
                return Ok(request["params"].clone());
            }
        }
    }

    async fn recv(&mut self) -> Option<serde_json::Value> {
        if let Some(value) = self.buffered.pop_front() { return Some(value); }
        loop {
            let request = self.requests.recv().await?;
            self.host.reply(self.generation, json!({"id":request["id"],"result":null})).unwrap();
            if request["method"] == "peerNotificationsChanged" {
                return Some(request["params"].clone());
            }
        }
    }
    async fn recv_clipboard(&mut self) -> Option<serde_json::Value> {
        loop {
            let request = self.requests.recv().await?;
            self.host.reply(self.generation, json!({"id":request["id"],"result":null})).unwrap();
            match request["method"].as_str() {
                Some("peerClipboardChanged") => return Some(request["params"].clone()),
                Some("peerNotificationsChanged") => self.buffered.push_back(request["params"].clone()),
                _ => {}
            }
        }
    }

}

fn fixture() -> (Store, DPeer, Identity, Arc<crate::host::bridge::Host>, CallbackReceiver) {
    let peer = DPeer::new("peer", "Fixture", "127.0.0.1", Some(18443), DeviceType::Computer);
    let identity = Identity {
        url: "wss://127.0.0.1:18443/?cid=fixture".into(),
        token: plain_rs::utils::base64::base64_encode(&[7; 32]),
        ip: peer.ip.clone(), port: peer.port,
    };
    let host = Arc::new(crate::host::bridge::Host::default());
    let receiver = CallbackReceiver::new(host.clone());
    let store = Store::default();
    store.reconcile(std::slice::from_ref(&peer), &HashMap::from([(peer.id.clone(), identity.clone())]), &host);
    (store, peer, identity, host, receiver)
}

fn notification(id: &str) -> Notification {
    Notification {
        id: id.into(),
        only_once: false,
        is_clearable: true,
        app_id: "fixture.app".into(),
        app_name: "Fixture".into(),
        posted_at: Instant(chrono::DateTime::from_timestamp(0, 0).unwrap()),
        silent: false,
        title: id.into(),
        body: "Body".into(),
        actions: vec![],
        reply_actions: vec![],
    }
}

fn observation(items: Vec<Notification>) -> Result<(App, Vec<Notification>, reqwest::Url)> {
    Ok((
        App {
            device_name: "Fixture".into(),
            capabilities: vec!["NOTIFICATIONS".into()],
            permissions: vec!["NOTIFICATION_LISTENER".into()],
            url_token: plain_rs::utils::base64::base64_encode(&[9; 32]),
        },
        items,
        reqwest::Url::parse("https://127.0.0.1:18443").unwrap(),
    ))
}

fn event(receiver: &mut CallbackReceiver) -> serde_json::Value {
    receiver.try_recv().unwrap()
}

#[tokio::test]
async fn baseline_diffs_and_repeated_hints_only_alert_new_confirmed_items() {
    let (store, peer, identity, events, mut receiver) = fixture();
    assert!(event(&mut receiver)["group"]["loading"].as_bool().unwrap());
    store
        .apply(
            &peer.id,
            &identity,
            observation(vec![notification("old")]),
            &events,
        )
        .unwrap();
    let baseline = event(&mut receiver);
    assert!(baseline["alerts"].as_array().unwrap().is_empty());
    assert_eq!(baseline["group"]["deviceType"], "COMPUTER");
    let icon = baseline["group"]["items"][0]["icon"].clone();
    store
        .apply(
            &peer.id,
            &identity,
            observation(vec![notification("old")]),
            &events,
        )
        .unwrap();
    assert!(receiver.try_recv().is_err());
    let mut updated = notification("old");
    updated.body = "Updated".into();
    store
        .apply(
            &peer.id,
            &identity,
            observation(vec![notification("new"), updated]),
            &events,
        )
        .unwrap();
    let update = event(&mut receiver);
    assert_eq!(update["alerts"].as_array().unwrap().len(), 1);
    assert_eq!(update["alerts"][0]["id"], "new");
    assert_eq!(update["group"]["items"][0]["icon"], icon);
    assert_eq!(update["group"]["items"][1]["body"], "Updated");
    store
        .apply(&peer.id, &identity, observation(vec![]), &events)
        .unwrap();
    assert!(
        event(&mut receiver)["group"]["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn silent_items_and_active_updates_do_not_alert_but_a_dismissed_id_can_notify_again() {
    let (store, peer, identity, events, mut receiver) = fixture();
    event(&mut receiver);
    let mut once = notification("once");
    once.only_once = true;
    store
        .apply(
            &peer.id,
            &identity,
            observation(vec![once.clone()]),
            &events,
        )
        .unwrap();
    event(&mut receiver);
    store
        .apply(
            &peer.id,
            &identity,
            Err(anyhow::anyhow!("offline")),
            &events,
        )
        .unwrap();
    let offline = event(&mut receiver);
    assert_eq!(offline["group"]["online"], false);
    assert_eq!(offline["group"]["items"][0]["id"], "once");
    once.body = "Updated active notification".into();
    let mut silent = notification("silent");
    silent.silent = true;
    store
        .apply(
            &peer.id,
            &identity,
            observation(vec![once.clone(), silent]),
            &events,
        )
        .unwrap();
    assert!(
        event(&mut receiver)["alerts"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    store
        .apply(&peer.id, &identity, observation(vec![]), &events)
        .unwrap();
    event(&mut receiver);
    store
        .apply(&peer.id, &identity, observation(vec![once]), &events)
        .unwrap();
    assert_eq!(event(&mut receiver)["alerts"][0]["id"], "once");
    store
        .apply(
            &peer.id,
            &identity,
            observation(vec![notification("ordinary")]),
            &events,
        )
        .unwrap();
    assert_eq!(event(&mut receiver)["alerts"][0]["id"], "ordinary");
}

#[tokio::test]
async fn capability_or_permission_revocation_clears_data_and_regrant_starts_a_fresh_baseline() {
    for capability in [true, false] {
        let (store, peer, identity, events, mut receiver) = fixture();
        event(&mut receiver);
        store
            .apply(
                &peer.id,
                &identity,
                observation(vec![notification("old")]),
                &events,
            )
            .unwrap();
        event(&mut receiver);
        let (mut app, _, base) = observation(vec![]).unwrap();
        if capability {
            app.capabilities.clear()
        } else {
            app.permissions.clear()
        }
        store
            .apply(&peer.id, &identity, Ok((app, vec![], base)), &events)
            .unwrap();
        let revoked = event(&mut receiver);
        assert!(revoked["group"]["items"].as_array().unwrap().is_empty());
        assert_eq!(revoked["group"]["online"], true);
        store
            .apply(
                &peer.id,
                &identity,
                observation(vec![notification("after-regrant")]),
                &events,
            )
            .unwrap();
        assert!(
            event(&mut receiver)["alerts"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
}

#[tokio::test]
async fn snapshots_recover_without_alerts_and_credentials_or_membership_retire_old_results() {
    let (store, mut peer, identity, events, mut receiver) = fixture();
    event(&mut receiver);
    store
        .apply(
            &peer.id,
            &identity,
            observation(vec![notification("old")]),
            &events,
        )
        .unwrap();
    event(&mut receiver);
    let initial: serde_json::Value = store.snapshot();
    assert_eq!(initial["groups"][0]["items"][0]["id"], "old");
    assert!(initial.get("alerts").is_none());
    peer.name = "Renamed".into();
    store.reconcile(
        std::slice::from_ref(&peer),
        &HashMap::from([(peer.id.clone(), identity.clone())]),
        &events,
    );
    let renamed = event(&mut receiver);
    assert_eq!(renamed["group"]["name"], "Renamed");
    assert_eq!(renamed["group"]["items"][0]["id"], "old");
    let mut moved = identity.clone();
    moved.ip = "127.0.0.2".into();
    moved.url = "wss://127.0.0.2:18443/?cid=fixture".into();
    store.reconcile(
        std::slice::from_ref(&peer),
        &HashMap::from([(peer.id.clone(), moved.clone())]),
        &events,
    );
    assert!(receiver.try_recv().is_err());
    let retained: serde_json::Value =
        store.snapshot();
    assert_eq!(retained["groups"][0]["items"][0]["id"], "old");
    store
        .apply(
            &peer.id,
            &identity,
            observation(vec![notification("stale-address")]),
            &events,
        )
        .unwrap();
    assert!(receiver.try_recv().is_err());
    let (app, items, _) =
        observation(vec![notification("old"), notification("fresh-after-move")]).unwrap();
    store
        .apply(
            &peer.id,
            &moved,
            Ok((
                app,
                items,
                reqwest::Url::parse("https://127.0.0.2:18443").unwrap(),
            )),
            &events,
        )
        .unwrap();
    let moved_event = event(&mut receiver);
    assert_eq!(moved_event["alerts"][0]["id"], "fresh-after-move");
    assert!(
        moved_event["group"]["items"][0]["icon"]
            .as_str()
            .unwrap()
            .starts_with("https://127.0.0.2:18443/")
    );
    let mut replacement = moved.clone();
    replacement.token = plain_rs::utils::base64::base64_encode(&[8; 32]);
    store.reconcile(
        std::slice::from_ref(&peer),
        &HashMap::from([(peer.id.clone(), replacement)]),
        &events,
    );
    assert!(
        event(&mut receiver)["group"]["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    store
        .apply(
            &peer.id,
            &identity,
            observation(vec![notification("stale")]),
            &events,
        )
        .unwrap();
    assert!(receiver.try_recv().is_err());
    store.reconcile(&[], &HashMap::new(), &events);
    assert!(event(&mut receiver)["group"].is_null());
    store
        .apply(
            &peer.id,
            &identity,
            observation(vec![notification("removed")]),
            &events,
        )
        .unwrap();
    assert!(receiver.try_recv().is_err());
}

#[tokio::test]
async fn invalid_snapshots_do_not_mutate_projection_and_icons_decrypt_with_standard_url_token() {
    let (store, peer, identity, events, mut receiver) = fixture();
    event(&mut receiver);
    let duplicate = notification("duplicate");
    assert!(
        store
            .apply(
                &peer.id,
                &identity,
                observation(vec![duplicate.clone(), duplicate]),
                &events
            )
            .is_err()
    );
    assert!(receiver.try_recv().is_err());
    let (mut app, items, base) = observation(vec![notification("valid")]).unwrap();
    app.url_token = plain_rs::utils::base64::base64_encode(&[9; 31]);
    assert!(
        store
            .apply(&peer.id, &identity, Ok((app, items, base)), &events)
            .is_err()
    );
    assert!(receiver.try_recv().is_err());
    store
        .apply(
            &peer.id,
            &identity,
            observation(vec![notification("valid")]),
            &events,
        )
        .unwrap();
    let value = event(&mut receiver);
    let url = reqwest::Url::parse(value["group"]["items"][0]["icon"].as_str().unwrap()).unwrap();
    let file_id = url
        .query_pairs()
        .find(|(key, _)| key == "id")
        .unwrap()
        .1
        .into_owned();
    let bytes = plain_rs::utils::base64::base64_decode(&file_id);
    assert_eq!(
        plain_rs::crypto::xchacha_decrypt_raw(&[9; 32], &bytes).unwrap(),
        b"pkgicon://fixture.app"
    );
    assert!(value["alerts"].as_array().unwrap().is_empty());
    assert!(!store.snapshot().to_string().contains("urlToken"));
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn encrypted_snapshots_keep_socket_delivery_live_and_discard_revoked_credentials() {
    use crate::assembly::server::ContentServer;
    use plain_rs::db::chat_store::{SaveMode, peers};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    let remote_dir = tempfile::tempdir().unwrap();
    let remote = ContentServer::open_with_cache(
        &remote_dir.path().join("data"),
        &remote_dir.path().join("cache"),
    )
    .unwrap();
    let remote_state = remote.runtime_state();
    remote_state
        .prefs
        .set(
            plain_rs::prefs::pref_keys::DEVICE_NAME,
            "Authenticated peer",
        )
        .unwrap();
    remote_state
        .prefs
        .set("api_permissions", json!(["NOTIFICATION_LISTENER", "CLIPBOARD"]))
        .unwrap();
    let session = remote
        .native_management()
        .unwrap()
        .create_session("Fixture notification manager".into())
        .unwrap();
    remote_state.prefs.set("http_port", 0).unwrap();
    remote_state.prefs.set("https_port", 0).unwrap();
    let (_, port) = remote.start_configured_public().await.unwrap();
    let (generation, mut requests) = remote_state.host.connect();
    let facts = Arc::new(Mutex::new(vec![
        serde_json::to_value(notification("old")).unwrap(),
    ]));
    let allowed = Arc::new(AtomicBool::new(true));
    let supported = Arc::new(AtomicBool::new(true));
    let hold = Arc::new(AtomicBool::new(true));
    let requested = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let reads = Arc::new(AtomicUsize::new(0));
    let subscription_ready = Arc::new(Notify::new());
    let sdk_ready = subscription_ready.clone();
    let (
        sdk_facts,
        sdk_allowed,
        sdk_supported,
        sdk_hold,
        sdk_requested,
        sdk_release,
        sdk_reads,
        host,
    ) = (
        facts.clone(),
        allowed.clone(),
        supported.clone(),
        hold.clone(),
        requested.clone(),
        release.clone(),
        reads.clone(),
        remote_state.host.clone(),
    );
    let responder = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let (sdk_facts, sdk_allowed, sdk_supported, sdk_hold, sdk_requested, sdk_release, sdk_reads, sdk_ready, host) = (
                sdk_facts.clone(), sdk_allowed.clone(), sdk_supported.clone(), sdk_hold.clone(), sdk_requested.clone(), sdk_release.clone(), sdk_reads.clone(), sdk_ready.clone(), host.clone()
            );
            tokio::spawn(async move {
            let result = match request["method"].as_str().unwrap() {
                "systemScreenMirrorResetTouch" => json!(true),
                "systemCapabilities" => {
                    if sdk_supported.load(Ordering::SeqCst) {
                        json!(["NOTIFICATIONS", "CLIPBOARD"])
                    } else {
                        json!(["CLIPBOARD"])
                    }
                }
                "systemPermissionFacts" => {
                    let granted: serde_json::Map<String, serde_json::Value> =
                        request["params"]["permissions"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|name| {
                                (
                                    name.as_str().unwrap().to_owned(),
                                    json!(
                                        name == "CLIPBOARD" || (name == "NOTIFICATION_LISTENER"
                                            && sdk_allowed.load(Ordering::SeqCst))
                                    ),
                                )
                            })
                            .collect();
                    if granted.contains_key("CLIPBOARD") { sdk_ready.notify_one(); }
                    json!({"granted":granted})
                }
                "systemNotificationFacts" => {
                    sdk_reads.fetch_add(1, Ordering::SeqCst);
                    if sdk_hold.swap(false, Ordering::SeqCst) {
                        sdk_requested.notify_one();
                        sdk_release.notified().await;
                    }
                    json!(sdk_facts.lock().unwrap().clone())
                }
                method => panic!("Unexpected notification primitive {method}"),
            };
            host.reply(generation, json!({"id":request["id"],"result":result}))
                .unwrap();
            });
        }
    });
    let local_dir = tempfile::tempdir().unwrap();
    let local = ContentServer::open_with_cache(
        &local_dir.path().join("data"),
        &local_dir.path().join("cache"),
    )
    .unwrap();
    let state = local.runtime_state();
    state.management.shutdown().await;
    state.prefs.set("client_id", &session.client_id).unwrap();
    let mut peer = DPeer::new(
        "remote",
        "Fixture",
        "127.0.0.1",
        Some(port),
        DeviceType::Computer,
    );
    peer.token = session.token;
    peers::save(&state.db, &[peer.clone()], SaveMode::Insert).unwrap();
    let mut callbacks = CallbackReceiver::new(state.host.clone());
    state.management.start(state.clone()).unwrap();
    tokio::time::timeout(Duration::from_secs(10), requested.notified())
        .await
        .unwrap();
    assert_eq!(
        peers::get(&state.db, &peer.id).unwrap().unwrap().name,
        "Authenticated peer"
    );
    tokio::time::timeout(Duration::from_secs(10), subscription_ready.notified()).await.unwrap();
    remote_state
        .events
        .send(WsEvent::broadcast(
            "DEVICE_NAME_UPDATED",
            json!("Injected name").to_string(),
        ))
        .unwrap();
    remote_state
        .events
        .send(WsEvent::broadcast(
            "NOTIFICATION_CREATED",
            json!({"id":"forged-hint","title":"Never display"}).to_string(),
        ))
        .unwrap();
    remote_state
        .events
        .send(WsEvent::broadcast("NOTIFICATION_REFRESHED", String::new()))
        .unwrap();
    remote_state
        .events
        .send(WsEvent::broadcast(
            "CLIPBOARD_CHANGED",
            json!({"text":"hint", "sensitive":false, "time":1}).to_string(),
        ))
        .unwrap();
    let clipboard = tokio::time::timeout(Duration::from_secs(3), callbacks.recv_clipboard())
        .await.unwrap().unwrap();
    assert_eq!(clipboard, json!({"peerIds":["remote"],"refreshAll":false}));
    release.notify_one();
    let baseline = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let value = callbacks.recv().await.unwrap();
            if value["group"]["loaded"] == true { break value; }
        }
    }).await.unwrap();
    assert_eq!(baseline["group"]["items"][0]["id"], "old");
    assert!(baseline["alerts"].as_array().unwrap().is_empty());
    state.management.shutdown().await;
    assert_eq!(
        peers::get(&state.db, &peer.id).unwrap().unwrap().name,
        "Authenticated peer"
    );
    let identity = Identity {
        url: format!("wss://127.0.0.1:{port}/?cid={}", session.client_id),
        token: peer.token.clone(),
        ip: peer.ip.clone(),
        port: peer.port,
    };
    let (cancel, cancelled) = watch::channel(false);
    while callbacks.try_recv().is_ok() {}
    *facts.lock().unwrap() = vec![serde_json::to_value(notification("confirmed")).unwrap()];
    refresh(&state, &peer.id, &identity, &cancelled)
        .await
        .unwrap();
    let confirmed = event(&mut callbacks);
    assert_eq!(confirmed["alerts"][0]["id"], "confirmed");
    refresh(&state, &peer.id, &identity, &cancelled)
        .await
        .unwrap();
    assert!(callbacks.try_recv().is_err());
    let before = reads.load(Ordering::SeqCst);
    allowed.store(false, Ordering::SeqCst);
    refresh(&state, &peer.id, &identity, &cancelled)
        .await
        .unwrap();
    assert_eq!(reads.load(Ordering::SeqCst), before);
    assert!(
        event(&mut callbacks)["group"]["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    allowed.store(true, Ordering::SeqCst);
    supported.store(false, Ordering::SeqCst);
    refresh(&state, &peer.id, &identity, &cancelled)
        .await
        .unwrap();
    assert_eq!(reads.load(Ordering::SeqCst), before);
    let unsupported = event(&mut callbacks);
    assert!(
        !unsupported["group"]["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|capability| capability == "NOTIFICATIONS")
    );
    supported.store(true, Ordering::SeqCst);
    refresh(&state, &peer.id, &identity, &cancelled)
        .await
        .unwrap();
    event(&mut callbacks);
    remote_state
        .prefs
        .set("api_permissions", json!([]))
        .unwrap();
    let before = reads.load(Ordering::SeqCst);
    refresh(&state, &peer.id, &identity, &cancelled)
        .await
        .unwrap();
    let error = super::super::management_http::query(
        &state,
        &reqwest::Url::parse(&format!("https://127.0.0.1:{port}")).unwrap(),
        &session.client_id,
        &plain_rs::utils::base64::base64_decode(&peer.token),
        json!({"query":"{ notifications(query:\"\",offset:0,limit:1) { id } }"}),
        256 * 1024,
    )
    .await
    .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<super::super::management_http::AuthorizationError>(),
        Some(super::super::management_http::AuthorizationError::Permission)
    ));
    let denied = event(&mut callbacks);
    assert!(denied["group"]["items"].as_array().unwrap().is_empty());
    assert!(
        denied["group"]["permissions"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(reads.load(Ordering::SeqCst), before);
    remote_state
        .prefs
        .set("api_permissions", json!(["NOTIFICATION_LISTENER"]))
        .unwrap();
    hold.store(true, Ordering::SeqCst);
    let pending = refresh(&state, &peer.id, &identity, &cancelled);
    tokio::pin!(pending);
    tokio::select! {
        _ = &mut pending => panic!("Snapshot must wait for platform facts"),
        result = tokio::time::timeout(Duration::from_secs(10), requested.notified()) => result.unwrap(),
    }
    state.db.with_conn(|connection| {
        connection
            .execute("UPDATE peers SET token='' WHERE id=?1", [&peer.id])
            .unwrap()
    });
    release.notify_one();
    pending.await.unwrap();
    assert!(callbacks.try_recv().is_err());
    cancel.send_replace(true);
    let before = reads.load(Ordering::SeqCst);
    refresh(&state, &peer.id, &identity, &cancelled)
        .await
        .unwrap();
    assert_eq!(reads.load(Ordering::SeqCst), before);
    cancel.send_replace(false);
    state.db.with_conn(|connection| {
        connection
            .execute(
                "UPDATE peers SET token=?1 WHERE id=?2",
                [&peer.token, &peer.id],
            )
            .unwrap()
    });
    refresh(&state, &peer.id, &identity, &cancelled)
        .await
        .unwrap();
    let restored = event(&mut callbacks);
    assert_eq!(restored["group"]["items"][0]["id"], "confirmed");
    assert_eq!(
        remote_state.db.session_delete(&session.client_id).unwrap(),
        1
    );
    refresh(&state, &peer.id, &identity, &cancelled)
        .await
        .unwrap();
    let rejected = event(&mut callbacks);
    assert_eq!(rejected["group"]["online"], false);
    assert!(rejected["group"]["items"].as_array().unwrap().is_empty());
    remote_state.host.disconnect(generation);
    responder.abort();
    local.stop_core().await;
    remote.stop_core().await;
}

#[tokio::test]
async fn confirmed_authorization_failures_clear_data_instead_of_looking_like_network_timeouts() {
    use super::super::management_http::AuthorizationError;
    for permission in [true, false] {
        let (store, peer, identity, events, mut receiver) = fixture();
        event(&mut receiver);
        store
            .apply(
                &peer.id,
                &identity,
                observation(vec![notification("old")]),
                &events,
            )
            .unwrap();
        event(&mut receiver);
        let error = if permission {
            AuthorizationError::Permission
        } else {
            AuthorizationError::Session
        };
        store
            .apply(&peer.id, &identity, Err(error.into()), &events)
            .unwrap();
        let denied = event(&mut receiver);
        assert_eq!(denied["group"]["online"], permission);
        assert!(denied["group"]["items"].as_array().unwrap().is_empty());
        store
            .apply(
                &peer.id,
                &identity,
                observation(vec![notification("after-login")]),
                &events,
            )
            .unwrap();
        assert!(
            event(&mut receiver)["alerts"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
}

#[tokio::test]
async fn notifications_arriving_after_snapshot_retain_their_host_callback() {
    let (store, peer, identity, host, mut callbacks) = fixture();
    event(&mut callbacks);
    store.apply(&peer.id, &identity, observation(vec![notification("old")]), &host).unwrap();
    event(&mut callbacks);
    let initial = store.snapshot();
    store.apply(&peer.id, &identity, observation(vec![notification("new"), notification("old")]), &host).unwrap();
    let update = event(&mut callbacks);
    assert_eq!(initial["groups"][0]["items"][0]["id"], "old");
    assert!(update["revision"].as_u64().unwrap() > initial["revision"].as_u64().unwrap());
    assert_eq!(update["alerts"][0]["id"], "new");
}

#[tokio::test]
async fn reconnecting_host_receives_a_complete_notification_snapshot_without_alerts() {
    let (store, peer, identity, host, mut old) = fixture();
    event(&mut old);
    store.apply(&peer.id, &identity, observation(vec![notification("old")]), &host).unwrap();
    event(&mut old);
    let mut replacement = CallbackReceiver::new(host.clone());
    store.notify_snapshot(&host).unwrap();
    let snapshot = event(&mut replacement);
    assert_eq!(snapshot["groups"][0]["items"][0]["id"], "old");
    assert!(snapshot.get("alerts").is_none());
    store.apply(&peer.id, &identity, observation(vec![notification("new")]), &host).unwrap();
    let update = event(&mut replacement);
    assert!(update["revision"].as_u64().unwrap() > snapshot["revision"].as_u64().unwrap());
    assert_eq!(update["alerts"][0]["id"], "new");
}

#[tokio::test]
async fn saturated_host_recovers_the_latest_snapshot_and_missed_alerts() {
    let (store, peer, identity, host, mut callbacks) = fixture();
    event(&mut callbacks);
    store.apply(&peer.id, &identity, observation(vec![notification("old")]), &host).unwrap();
    event(&mut callbacks);
    for _ in 0..crate::host::bridge::REQUEST_CAPACITY {
        host.notify("blocked", json!({})).unwrap();
    }
    store.apply(&peer.id, &identity, observation(vec![notification("new")]), &host).unwrap();
    assert!(callbacks.try_recv().is_err());
    store.apply(&peer.id, &identity, observation(vec![notification("latest")]), &host).unwrap();
    let recovery = tokio::time::timeout(Duration::from_secs(3), callbacks.recv()).await.unwrap().unwrap();
    assert_eq!(recovery["groups"][0]["items"][0]["id"], "latest");
    assert_eq!(recovery["alerts"].as_array().unwrap().iter().map(|item| item["id"].as_str().unwrap()).collect::<Vec<_>>(), ["new", "latest"]);
}
#[tokio::test]
async fn state_changes_before_host_attachment_are_delivered_as_the_initial_snapshot() {
    let (store, peer, identity, host, mut callbacks) = fixture();
    event(&mut callbacks);
    host.disconnect(callbacks.generation);
    drop(callbacks);
    let before = host.callback_attempts.load(std::sync::atomic::Ordering::Relaxed);
    store.apply(&peer.id, &identity, observation(vec![notification("before-attach")]), &host).unwrap();
    assert_eq!(host.callback_attempts.load(std::sync::atomic::Ordering::Relaxed), before);
    let mut current = CallbackReceiver::new(host.clone());
    store.notify_snapshot(&host).unwrap();
    let snapshot = event(&mut current);
    assert_eq!(snapshot["groups"][0]["items"][0]["id"], "before-attach");
    assert!(snapshot.get("alerts").is_none());
    store.apply(&peer.id, &identity, observation(vec![notification("after-attach")]), &host).unwrap();
    let update = event(&mut current);
    assert_eq!(update["alerts"][0]["id"], "after-attach");
    assert!(update["revision"].as_u64().unwrap() > snapshot["revision"].as_u64().unwrap());
    host.disconnect(current.generation);
}
