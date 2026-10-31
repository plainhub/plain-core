use futures_util::SinkExt;
use futures_util::StreamExt;
use plain_rs::db::DPeer;
use plain_rs::db::chat_store::peers;
use plain_rs::events::WsEvent;
use plain_rs::runtime::Runtime as ServerState;
use plain_rs::services::peers::clipboard::receive as synchronize_clipboard;
use plain_rs::services::peers::management::*;
use plain_rs::{chat::enums::DeviceType, db::chat_store::SaveMode};
use serde_json::json;
use std::time::Duration;
use tokio::sync::watch;

async fn fixture() -> (tempfile::TempDir, crate::server::ContentServer, ServerState) {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open_with_cache(
        &directory.path().join("data"),
        &directory.path().join("cache"),
    )
    .unwrap();
    let state = core.runtime_state().core;
    state.management.shutdown().await;
    (directory, core, state)
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn actual_management_socket_authenticates_and_relays_only_decrypted_native_events() {
    let remote_directory = tempfile::tempdir().unwrap();
    let remote = crate::server::ContentServer::open_with_cache(
        &remote_directory.path().join("data"),
        &remote_directory.path().join("cache"),
    )
    .unwrap();
    let remote_state = remote.runtime_state().core;
    remote_state
        .prefs
        .set("api_permissions", vec!["CLIPBOARD"])
        .unwrap();
    let remote_host = remote_state.host.clone();
    let (generation, mut remote_requests) = remote_host.connect();
    let (ready, mut ready_rx) = tokio::sync::mpsc::channel(4);
    let responder = tokio::spawn(async move {
        while let Some(request) = remote_requests.recv().await {
            let method = request["method"].as_str().unwrap_or_default();
            let result = match method {
                "systemCapabilities" => json!(["CLIPBOARD"]),
                "systemPermissionFacts" => {
                    let granted = request["params"]["permissions"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter_map(|name| name.as_str())
                        .map(|name| (name.to_owned(), json!(true)))
                        .collect::<serde_json::Map<_, _>>();
                    if granted.contains_key("CLIPBOARD") {
                        let _ = ready.try_send(());
                    }
                    json!({"granted":granted})
                }
                _ => json!({}),
            };
            let _ = remote_host.reply(generation, json!({"id":request["id"],"result":result}));
        }
    });
    let session = remote
        .native_management()
        .unwrap()
        .create_session("Fixture manager".into())
        .unwrap();
    remote_state.prefs.set("http_port", 0).unwrap();
    remote_state.prefs.set("https_port", 0).unwrap();
    let (_, tls_port) = remote.start_configured_public().await.unwrap();
    let directory = tempfile::tempdir().unwrap();
    let data = directory.path().join("data");
    std::fs::create_dir_all(&data).unwrap();
    let prefs = plain_rs::prefs::Prefs::load(&plain_rs::prefs::default_path(&data)).unwrap();
    prefs
        .set(plain_rs::prefs::pref_keys::CLIENT_ID, &session.client_id)
        .unwrap();
    let manager =
        crate::server::ContentServer::open_with_cache(&data, &directory.path().join("cache"))
            .unwrap();
    let state = manager.runtime_state().core;
    let (host_generation, mut requests) = state.host.connect();
    let mut peer = DPeer::new(
        &remote_state.native_client_id().unwrap(),
        "Fixture server",
        "127.0.0.1",
        Some(tls_port),
        DeviceType::Computer,
    );
    peer.token = session.token;
    peers::save(&state.db, &[peer.clone()], SaveMode::Insert).unwrap();
    tokio::time::timeout(Duration::from_secs(10), ready_rx.recv())
        .await
        .unwrap()
        .unwrap();
    let payload = json!({"text":"fixture clipboard hint", "sensitive":false, "time":1});
    let relayed = tokio::time::timeout(Duration::from_secs(10), async {
        let mut ticks = tokio::time::interval(Duration::from_millis(20));
        loop {
            tokio::select! {
                _ = ticks.tick() => {
                    remote_state.events.send(WsEvent::broadcast("CLIPBOARD_CHANGED", payload.to_string())).unwrap();
                }
                request = requests.recv() => {
                    let request = request.unwrap();
                    state.host.reply(host_generation, json!({"id":request["id"],"result":null})).unwrap();
                    if request["method"] == "peerClipboardChanged" { break request; }
                }
            }
        }
    }).await.unwrap();
    assert_eq!(
        relayed["params"],
        json!({"peerIds":[peer.id],"refreshAll":false})
    );
    state.host.disconnect(host_generation);
    manager.stop_core().await;
    responder.abort();
    remote.stop_core().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn encrypted_clipboard_sync_seeds_baseline_retries_failed_writes_and_rechecks_grants() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    fn seed(db: &plain_rs::db::Db, text: &str, sensitive: bool, sequence: u8) {
        let (item, _) = plain_rs::clipboard::record(db, text, "", "", sensitive).unwrap();
        let time = format!("2026-01-01T00:00:{sequence:02}.000Z");
        db.with_conn(|connection| {
            connection
                .execute(
                    "UPDATE clipboards SET created_at=?1 WHERE id=?2",
                    [&time, &item.id],
                )
                .unwrap()
        });
    }
    let remote_directory = tempfile::tempdir().unwrap();
    let remote = crate::server::ContentServer::open_with_cache(
        &remote_directory.path().join("data"),
        &remote_directory.path().join("cache"),
    )
    .unwrap();
    let remote_state = remote.runtime_state().core;
    remote_state
        .prefs
        .set("api_permissions", json!(["CLIPBOARD"]))
        .unwrap();
    let session = remote
        .native_management()
        .unwrap()
        .create_session("Fixture sync".into())
        .unwrap();
    remote_state.prefs.set("http_port", 0).unwrap();
    remote_state.prefs.set("https_port", 0).unwrap();
    let (_, port) = remote.start_configured_public().await.unwrap();
    let (_directory, manager, state) = fixture().await;
    state.prefs.set("client_id", &session.client_id).unwrap();
    let mut peer = DPeer::new(
        "remote-clipboard",
        "Fixture",
        "127.0.0.1",
        Some(port),
        DeviceType::Computer,
    );
    peer.token = session.token;
    peers::save(&state.db, &[peer.clone()], SaveMode::Insert).unwrap();
    let expected = identity(
        &peer,
        &session.client_id,
        &format!("https://127.0.0.1:{port}"),
    )
    .unwrap();
    let (cancel, cancelled) = watch::channel(false);
    let (generation, mut requests) = crate::test_support::HostRequests::connect(&state.host);
    let writes = Arc::new(AtomicUsize::new(0));
    let succeeds = Arc::new(AtomicBool::new(false));
    let supports = Arc::new(AtomicBool::new(true));
    let (count, success, support, host) = (
        writes.clone(),
        succeeds.clone(),
        supports.clone(),
        state.host.clone(),
    );
    let responder = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let result = match request["method"].as_str().unwrap() {
                "systemCapabilities" => {
                    if support.load(Ordering::SeqCst) {
                        json!(["CLIPBOARD"])
                    } else {
                        json!([])
                    }
                }
                "systemSetClipboard" => {
                    count.fetch_add(1, Ordering::SeqCst);
                    assert_eq!(request["params"]["text"], "new clipboard");
                    json!(success.load(Ordering::SeqCst))
                }
                method => panic!("Unexpected host primitive {method}"),
            };
            host.reply(generation, json!({"id":request["id"],"result":result}))
                .unwrap();
        }
    });
    seed(&remote_state.db, "old clipboard", false, 1);
    synchronize_clipboard(&state, &peer.id, &expected, &cancelled)
        .await
        .unwrap();
    assert_eq!(writes.load(Ordering::SeqCst), 0);
    seed(&remote_state.db, "new clipboard", false, 2);
    assert!(
        synchronize_clipboard(&state, &peer.id, &expected, &cancelled)
            .await
            .is_err()
    );
    assert_eq!(writes.load(Ordering::SeqCst), 1);
    succeeds.store(true, Ordering::SeqCst);
    synchronize_clipboard(&state, &peer.id, &expected, &cancelled)
        .await
        .unwrap();
    synchronize_clipboard(&state, &peer.id, &expected, &cancelled)
        .await
        .unwrap();
    assert_eq!(writes.load(Ordering::SeqCst), 2);
    let history = state.db.clipboard_page("", 10, 0).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].text, "new clipboard");
    assert_eq!(history[0].source, peer.id);
    state
        .prefs
        .set("api_permissions", json!(["CLIPBOARD"]))
        .unwrap();
    let mut echoes = state.events.subscribe();
    plain_rs::services::clipboard::report(
        &state,
        &json!({"text":"new clipboard","sensitive":false}).to_string(),
    )
    .unwrap();
    assert!(echoes.try_recv().is_err());
    seed(&remote_state.db, "private clipboard", true, 3);
    synchronize_clipboard(&state, &peer.id, &expected, &cancelled)
        .await
        .unwrap();
    assert_eq!(writes.load(Ordering::SeqCst), 2);
    seed(&remote_state.db, "must not apply", false, 4);
    supports.store(false, Ordering::SeqCst);
    synchronize_clipboard(&state, &peer.id, &expected, &cancelled)
        .await
        .unwrap();
    supports.store(true, Ordering::SeqCst);
    state
        .prefs
        .set("clipboard_directions", json!({peer.id.clone():"off"}))
        .unwrap();
    synchronize_clipboard(&state, &peer.id, &expected, &cancelled)
        .await
        .unwrap();
    state.prefs.set("clipboard_directions", json!({})).unwrap();
    cancel.send_replace(true);
    synchronize_clipboard(&state, &peer.id, &expected, &cancelled)
        .await
        .unwrap();
    cancel.send_replace(false);
    state.db.with_conn(|connection| {
        connection
            .execute("UPDATE peers SET token='' WHERE id=?1", [&peer.id])
            .unwrap()
    });
    synchronize_clipboard(&state, &peer.id, &expected, &cancelled)
        .await
        .unwrap();
    assert_eq!(writes.load(Ordering::SeqCst), 2);
    assert_eq!(state.db.clipboard_count_rows().unwrap(), 1);
    let (remote_generation, mut remote_requests) =
        crate::test_support::HostRequests::connect(&remote_state.host);
    let (remote_host, remote_db) = (remote_state.host.clone(), remote_state.db.clone());
    let expected_source = session.client_id.clone();
    let remote_responder = tokio::spawn(async move {
        while let Some(request) = remote_requests.recv().await {
            let result = match request["method"].as_str().unwrap() {
                "systemCapabilities" => json!(["CLIPBOARD"]),
                "systemSetClipboard" => {
                    assert_eq!(request["params"]["text"], "encrypted fixture write");
                    let item = remote_db
                        .clipboard_page("encrypted fixture write", 1, 0)
                        .unwrap();
                    assert_eq!(item[0].source, expected_source);
                    json!(true)
                }
                method => panic!("Unexpected remote host primitive {method}"),
            };
            remote_host
                .reply(
                    remote_generation,
                    json!({"id":request["id"],"result":result}),
                )
                .unwrap();
        }
    });
    let result = plain_rs::services::peers::management_http::query(
        &state,
        &reqwest::Url::parse(&format!("https://127.0.0.1:{port}")).unwrap(),
        &session.client_id,
        &plain_rs::utils::base64::base64_decode(&peer.token),
        json!({"query":"mutation($text:String!){setClipboard(text:$text)}", "variables":{"text":"encrypted fixture write"}}),
        4 * 1024 * 1024,
    ).await.unwrap();
    assert_eq!(result["setClipboard"], true);
    let mut remote_echoes = remote_state.events.subscribe();
    plain_rs::services::clipboard::report(
        &remote_state,
        &json!({"text":"encrypted fixture write","sensitive":false}).to_string(),
    )
    .unwrap();
    assert!(remote_echoes.try_recv().is_err());
    remote_state.host.disconnect(remote_generation);
    remote_responder.abort();
    state.host.disconnect(generation);
    responder.abort();
    manager.stop_core().await;
    remote.stop_core().await;
}
