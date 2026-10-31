use anyhow::Result;
use plain_rs::chat::enums::DeviceType;
use plain_rs::db::DPeer;
use plain_rs::services::notifications::types::Notification;
use plain_rs::services::peers::management::Identity;
use serde_json::json;
use std::sync::Mutex;
use tokio::sync::watch;

use plain_rs::events::WsEvent;
use plain_rs::types::Instant;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

struct CallbackReceiver {
    host: Arc<plain_rs::host::bridge::Host>,
    generation: u64,
    requests: tokio::sync::mpsc::Receiver<serde_json::Value>,
    buffered: std::collections::VecDeque<serde_json::Value>,
}

impl CallbackReceiver {
    fn new(host: Arc<plain_rs::host::bridge::Host>) -> Self {
        let (generation, requests) = host.connect();
        Self {
            host,
            generation,
            requests,
            buffered: Default::default(),
        }
    }

    fn try_recv(&mut self) -> Result<serde_json::Value, tokio::sync::mpsc::error::TryRecvError> {
        if let Some(value) = self.buffered.pop_front() {
            return Ok(value);
        }
        loop {
            let request = self.requests.try_recv()?;
            self.host
                .reply(self.generation, json!({"id":request["id"],"result":null}))
                .unwrap();
            if request["method"] == "peerNotificationsChanged" {
                return Ok(request["params"].clone());
            }
        }
    }

    async fn recv(&mut self) -> Option<serde_json::Value> {
        if let Some(value) = self.buffered.pop_front() {
            return Some(value);
        }
        loop {
            let request = self.requests.recv().await?;
            self.host
                .reply(self.generation, json!({"id":request["id"],"result":null}))
                .unwrap();
            if request["method"] == "peerNotificationsChanged" {
                return Some(request["params"].clone());
            }
        }
    }
    async fn recv_clipboard(&mut self) -> Option<serde_json::Value> {
        loop {
            let request = self.requests.recv().await?;
            self.host
                .reply(self.generation, json!({"id":request["id"],"result":null}))
                .unwrap();
            match request["method"].as_str() {
                Some("peerClipboardChanged") => return Some(request["params"].clone()),
                Some("peerNotificationsChanged") => {
                    self.buffered.push_back(request["params"].clone())
                }
                _ => {}
            }
        }
    }
}

async fn refresh(
    state: &plain_rs::runtime::Runtime,
    id: &str,
    identity: &plain_rs::services::peers::management::Identity,
    cancelled: &tokio::sync::watch::Receiver<bool>,
) -> anyhow::Result<()> {
    plain_rs::services::peers::management::refresh_data(state, id, identity, cancelled, None).await
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

fn event(receiver: &mut CallbackReceiver) -> serde_json::Value {
    receiver.try_recv().unwrap()
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn encrypted_snapshots_keep_socket_delivery_live_and_discard_revoked_credentials() {
    use crate::server::ContentServer;
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
    let remote_state = remote.runtime_state().core;
    remote_state
        .prefs
        .set(
            plain_rs::prefs::pref_keys::DEVICE_NAME,
            "Authenticated peer",
        )
        .unwrap();
    remote_state
        .prefs
        .set(
            "api_permissions",
            json!(["NOTIFICATION_LISTENER", "CLIPBOARD"]),
        )
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
            let (
                sdk_facts,
                sdk_allowed,
                sdk_supported,
                sdk_hold,
                sdk_requested,
                sdk_release,
                sdk_reads,
                sdk_ready,
                host,
            ) = (
                sdk_facts.clone(),
                sdk_allowed.clone(),
                sdk_supported.clone(),
                sdk_hold.clone(),
                sdk_requested.clone(),
                sdk_release.clone(),
                sdk_reads.clone(),
                sdk_ready.clone(),
                host.clone(),
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
                        let granted: serde_json::Map<String, serde_json::Value> = request["params"]
                            ["permissions"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|name| {
                                (
                                    name.as_str().unwrap().to_owned(),
                                    json!(
                                        name == "CLIPBOARD"
                                            || (name == "NOTIFICATION_LISTENER"
                                                && sdk_allowed.load(Ordering::SeqCst))
                                    ),
                                )
                            })
                            .collect();
                        if granted.contains_key("CLIPBOARD") {
                            sdk_ready.notify_one();
                        }
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
    let state = local.runtime_state().core;
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
    tokio::time::timeout(Duration::from_secs(10), subscription_ready.notified())
        .await
        .unwrap();
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
        .await
        .unwrap()
        .unwrap();
    assert_eq!(clipboard, json!({"peerIds":["remote"],"refreshAll":false}));
    release.notify_one();
    let baseline = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let value = callbacks.recv().await.unwrap();
            if value["group"]["loaded"] == true {
                break value;
            }
        }
    })
    .await
    .unwrap();
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
    let error = plain_rs::services::peers::management_http::query(
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
        error.downcast_ref::<plain_rs::services::peers::management_http::AuthorizationError>(),
        Some(plain_rs::services::peers::management_http::AuthorizationError::Permission)
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
