use futures_util::SinkExt;
use futures_util::StreamExt;
use plain_libs::serde_json::json;
use plain_rs::features::chat::enums::{DeviceType, PeerStatus};
use plain_rs::infrastructure::db::DPeer;
use plain_rs::infrastructure::db::chat_store::SaveMode;
use plain_rs::infrastructure::db::chat_store::peers;
use plain_rs::infrastructure::events::WsEvent;
use std::time::Duration;

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn actual_management_socket_authenticates_and_relays_only_decrypted_native_events() {
    let remote_directory = plain_libs::temporary::tempdir().unwrap();
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
                "systemAccessFacts" => {
                    let granted = request["params"]["features"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter_map(|name| name.as_str())
                        .map(|name| (name.to_owned(), json!(true)))
                        .collect::<plain_libs::serde_json::Map<_, _>>();
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
    let directory = plain_libs::temporary::tempdir().unwrap();
    let data = directory.path().join("data");
    std::fs::create_dir_all(&data).unwrap();
    let prefs =
        plain_rs::features::prefs::Prefs::load(&plain_rs::features::prefs::default_path(&data))
            .unwrap();
    prefs
        .set(
            plain_rs::features::prefs::pref_keys::CLIENT_ID,
            &session.client_id,
        )
        .unwrap();
    let manager =
        crate::server::ContentServer::open_with_cache(&data, &directory.path().join("cache"))
            .unwrap();
    let state = manager.runtime_state().core;
    state
        .prefs
        .set("api_permissions", json!(["CLIPBOARD"]))
        .unwrap();
    let (host_generation, mut requests) = state.host.connect();
    let mut peer = DPeer::new(
        &remote_state.native_client_id().unwrap(),
        "Fixture server",
        "127.0.0.1",
        Some(tls_port),
        DeviceType::Computer,
    );
    peer.status = PeerStatus::Paired;
    peer.token = session.token.clone();
    let mut login = remote_state
        .db
        .session_get(&session.client_id)
        .unwrap()
        .unwrap();
    login.r#type = plain_rs::features::sessions::SessionType::Login;
    remote_state.db.session_save(&login).unwrap();
    let mut reverse = DPeer::new(
        &session.client_id,
        "Fixture manager",
        "127.0.0.1",
        Some(tls_port),
        DeviceType::Computer,
    );
    reverse.status = PeerStatus::Paired;
    reverse.token = plain_libs::utils::base64::base64_encode(&[3; 32]);
    peers::save(&remote_state.db, &[reverse], SaveMode::Insert).unwrap();
    remote_state
        .prefs
        .set("clipboard_directions", json!({session.client_id:"SEND"}))
        .unwrap();
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
