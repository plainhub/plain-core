use super::*;
use crate::test_runtime::Core as ContentServer;
use plain_rs::chat::enums::DeviceType;
use plain_rs::chat::enums::PeerStatus;
use plain_rs::db::chat_store::peers;
use plain_rs::db::{DPeer, chat_store::SaveMode};
use plain_rs::runtime::Runtime as CoreRuntime;
use serde_json::Value;
use serde_json::json;
use std::sync::{Arc, Mutex};

fn server() -> (tempfile::TempDir, ContentServer, CoreRuntime) {
    let directory = tempfile::tempdir().unwrap();
    let prefs =
        Arc::new(plain_rs::prefs::Prefs::load(&directory.path().join("system.json")).unwrap());
    prefs.set("client_id", "local").unwrap();
    let server = ContentServer::start(
        directory.path(),
        &plain_rs::utils::base64::base64_encode(&[5; 32]),
        prefs,
    )
    .unwrap();
    let state = server.runtime_state();
    let mut peer = DPeer::new("remote", "remote", "", Some(443), DeviceType::Phone);
    peer.status = PeerStatus::Paired;
    peer.key = plain_rs::utils::base64::base64_encode(&[7; 32]);
    peers::save(&state.db, &[peer], SaveMode::Insert).unwrap();
    (directory, server, state)
}

fn host(state: &CoreRuntime) -> (Arc<Mutex<Vec<Value>>>, tokio::task::JoinHandle<()>) {
    let host = state.host.clone();
    let (generation, mut receiver) = host.connect();
    let received = Arc::new(Mutex::new(Vec::new()));
    let messages = received.clone();
    let task = tokio::spawn(async move {
        while let Some(request) = receiver.recv().await {
            if request["method"] == "systemCapabilities" {
                host.reply(
                    generation,
                    json!({"id":request["id"],"result":["WIFI_AWARE"]}),
                )
                .unwrap();
                continue;
            }
            let id = request["id"].clone();
            assert!(
                matches!(
                    request["method"].as_str(),
                    Some("peerStartAware" | "peerStopAware")
                ),
                "{request}"
            );
            messages.lock().unwrap().push(request);
            host.reply(generation, json!({"id":id,"result":true}))
                .unwrap();
        }
    });
    (received, task)
}

#[tokio::test]
async fn subscriptions_reuse_unchanged_parameters_and_refresh_live_key_role_and_ports() {
    let (_directory, server, state) = server();
    let (messages, task) = host(&state);
    let runtime = Runtime::default();
    assert!(runtime.subscribe(&state, "remote").await.unwrap());
    for _ in 0..4 {
        assert!(runtime.subscribe(&state, "remote").await.unwrap());
    }
    assert_eq!(messages.lock().unwrap().len(), 1);
    let first = messages.lock().unwrap()[0]["params"].clone();
    assert_eq!(
        first,
        json!({"peer":{"id":"remote","port":443},"config":{"pmk":plain_rs::utils::base64::base64_encode(&[7;32]),"isClient":true,"localPort":8443}})
    );
    let mut peer = peers::get(&state.db, "remote").unwrap().unwrap();
    peer.key = plain_rs::utils::base64::base64_encode(&[9; 32]);
    peer.port = Some(5443);
    peers::save(&state.db, &[peer], SaveMode::Update).unwrap();
    state.prefs.set("client_id", "z-local").unwrap();
    state.prefs.set("https_port", 6443).unwrap();
    runtime.refresh(&state).await.unwrap();
    assert_eq!(messages.lock().unwrap().len(), 2);
    assert_eq!(
        messages.lock().unwrap()[1]["params"],
        json!({"peer":{"id":"remote","port":5443},"config":{"pmk":plain_rs::utils::base64::base64_encode(&[9;32]),"isClient":false,"localPort":6443}})
    );
    runtime.refresh(&state).await.unwrap();
    assert_eq!(messages.lock().unwrap().len(), 2);
    runtime.stop(&state).await.unwrap();
    server.shutdown().await;
    task.abort();
}

#[tokio::test]
async fn unpair_removes_subscription_and_rolled_back_updates_do_not_reconfigure_sdk() {
    let (_directory, server, state) = server();
    let (messages, task) = host(&state);
    let runtime = Runtime::default();
    runtime.ensure(&state).await.unwrap();
    state.db.with_conn(|connection| {
        let transaction = connection.unchecked_transaction().unwrap();
        transaction
            .execute("UPDATE peers SET key='invalid' WHERE id='remote'", [])
            .unwrap();
        transaction.rollback().unwrap();
    });
    runtime.refresh(&state).await.unwrap();
    assert_eq!(messages.lock().unwrap().len(), 1);
    peers::unpair(&state.db, "remote").unwrap();
    runtime.refresh(&state).await.unwrap();
    assert_eq!(messages.lock().unwrap().len(), 2);
    assert_eq!(messages.lock().unwrap()[1]["method"], "peerStopAware");
    assert_eq!(
        messages.lock().unwrap()[1]["params"],
        json!({"id":"remote"})
    );
    runtime.refresh(&state).await.unwrap();
    assert_eq!(messages.lock().unwrap().len(), 2);
    server.shutdown().await;
    task.abort();
}

#[tokio::test]
async fn channel_only_peers_use_open_sdk_parameters_and_corrupt_keys_revoke_old_subscription() {
    let (_directory, server, state) = server();
    let (messages, task) = host(&state);
    let runtime = Runtime::default();
    let mut peer = peers::get(&state.db, "remote").unwrap().unwrap();
    peer.status = PeerStatus::Channel;
    peer.key.clear();
    peers::save(&state.db, &[peer.clone()], SaveMode::Update).unwrap();
    runtime.ensure(&state).await.unwrap();
    assert!(messages.lock().unwrap().is_empty());
    assert!(runtime.subscribe(&state, "remote").await.unwrap());
    assert!(messages.lock().unwrap()[0]["params"]["config"]["pmk"].is_null());
    peer.key = "%%%".to_owned();
    peers::save(&state.db, &[peer], SaveMode::Update).unwrap();
    assert!(runtime.refresh(&state).await.is_err());
    assert_eq!(messages.lock().unwrap().len(), 2);
    assert_eq!(messages.lock().unwrap()[1]["method"], "peerStopAware");
    server.shutdown().await;
    task.abort();
}

#[tokio::test]
async fn database_watch_pushes_key_changes_without_platform_graphql_queries() {
    let (_directory, server, state) = server();
    let host = state.host.clone();
    let (generation, mut requests) = host.connect();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let messages = observed.clone();
    let (sent, mut received) = tokio::sync::mpsc::unbounded_channel();
    let task = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            if request["method"] == "systemCapabilities" {
                host.reply(
                    generation,
                    json!({"id":request["id"],"result":["WIFI_AWARE"]}),
                )
                .unwrap();
                continue;
            }
            assert!(
                matches!(
                    request["method"].as_str(),
                    Some("peerStartAware" | "peerStopAware")
                ),
                "{request}"
            );
            messages.lock().unwrap().push(request.clone());
            host.reply(generation, json!({"id":request["id"],"result":true}))
                .unwrap();
            sent.send(request).unwrap();
        }
    });
    assert!(state.aware.subscribe(&state, "remote").await.unwrap());
    received.recv().await.unwrap();
    state
        .db
        .with_conn(|connection| {
            connection.execute("UPDATE peers SET port=5443 WHERE id='remote'", [])
        })
        .unwrap();
    let changed = tokio::time::timeout(std::time::Duration::from_secs(2), received.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(changed["params"]["peer"]["port"], 5443);
    state.aware.refresh(&state).await.unwrap();
    assert_eq!(observed.lock().unwrap().len(), 2);
    peers::delete(&state.db, &["remote".into()]).unwrap();
    let removed = tokio::time::timeout(std::time::Duration::from_secs(2), received.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(removed["method"], "peerStopAware");
    state.aware.refresh(&state).await.unwrap();
    assert_eq!(observed.lock().unwrap().len(), 3);
    server.shutdown().await;
    task.abort();
}

#[tokio::test]
async fn denied_start_is_not_repeated_by_refresh_but_explicit_retry_can_acquire_permission() {
    let (_directory, server, state) = server();
    let host = state.host.clone();
    let (generation, mut receiver) = host.connect();
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let calls = count.clone();
    let task = tokio::spawn(async move {
        while let Some(request) = receiver.recv().await {
            if request["method"] == "systemCapabilities" {
                host.reply(
                    generation,
                    json!({"id":request["id"],"result":["WIFI_AWARE"]}),
                )
                .unwrap();
                continue;
            }
            let attempt = calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            host.reply(
                generation,
                json!({"id":request["id"],"result":attempt != 0}),
            )
            .unwrap();
        }
    });
    let runtime = Runtime::default();
    assert!(!runtime.subscribe(&state, "remote").await.unwrap());
    for _ in 0..4 {
        runtime.refresh(&state).await.unwrap();
    }
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(runtime.subscribe(&state, "remote").await.unwrap());
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 2);
    runtime.stop(&state).await.unwrap();
    server.shutdown().await;
    task.abort();
}

#[tokio::test]
async fn host_replacement_resends_subscription_even_when_parameters_are_identical() {
    let (_directory, server, state) = server();
    let (first, first_task) = host(&state);
    let runtime = Runtime::default();
    assert!(runtime.subscribe(&state, "remote").await.unwrap());
    assert_eq!(first.lock().unwrap().len(), 1);
    let (replacement, second_task) = host(&state);
    runtime.refresh(&state).await.unwrap();
    assert_eq!(replacement.lock().unwrap().len(), 1);
    assert_eq!(
        first.lock().unwrap()[0]["params"],
        replacement.lock().unwrap()[0]["params"]
    );
    runtime.stop(&state).await.unwrap();
    server.shutdown().await;
    first_task.abort();
    second_task.abort();
}

#[tokio::test]
async fn permission_events_retry_rejected_subscriptions_without_a_page_command() {
    let (_directory, server, state) = server();
    let host = state.host.clone();
    let (generation, mut requests) = host.connect();
    let (sent, mut received) = tokio::sync::mpsc::unbounded_channel();
    let task = tokio::spawn(async move {
        let mut attempts = 0;
        while let Some(request) = requests.recv().await {
            if request["method"] == "systemCapabilities" {
                host.reply(
                    generation,
                    json!({"id":request["id"],"result":["WIFI_AWARE"]}),
                )
                .unwrap();
                continue;
            }
            assert!(matches!(
                request["method"].as_str(),
                Some("peerStartAware" | "peerStopAware")
            ));
            let result = request["method"] != "peerStartAware" || attempts > 0;
            if request["method"] == "peerStartAware" {
                attempts += 1;
            }
            host.reply(generation, json!({"id":request["id"],"result":result}))
                .unwrap();
            sent.send(request).unwrap();
        }
    });
    assert!(!state.aware.subscribe(&state, "remote").await.unwrap());
    let first = received.recv().await.unwrap();
    for _ in 0..4 {
        state.aware.refresh(&state).await.unwrap();
    }
    assert!(received.try_recv().is_err());
    state
        .events
        .send(plain_rs::events::WsEvent::host_text("PERMISSIONS_UPDATED", "[]".into()).unwrap())
        .unwrap();
    let restored = tokio::time::timeout(std::time::Duration::from_secs(2), received.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(restored["method"], "peerStartAware");
    assert_eq!(first["params"], restored["params"]);
    state.aware.refresh(&state).await.unwrap();
    assert!(received.try_recv().is_err());
    server.shutdown().await;
    task.abort();
}

#[tokio::test]
async fn permission_refresh_does_not_start_an_inactive_runtime() {
    let (_directory, server, state) = server();
    let (messages, task) = host(&state);
    let runtime = Runtime::default();
    runtime.permissions_changed(&state).await.unwrap();
    assert!(messages.lock().unwrap().is_empty());
    runtime.ensure(&state).await.unwrap();
    runtime.stop(&state).await.unwrap();
    messages.lock().unwrap().clear();
    runtime.permissions_changed(&state).await.unwrap();
    assert!(messages.lock().unwrap().is_empty());
    server.shutdown().await;
    task.abort();
}
