use plain_rs::features::discovery::nearby_devices::Device;
use plain_rs::features::services::discovery::nearby_devices::*;
use plain_rs::infrastructure::db::chat_store;
use plain_libs::serde_json::json;

#[tokio::test]
async fn rust_sweep_retires_unreachable_lan_records_without_a_host_bridge() {
    use std::sync::Arc;
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs =
        Arc::new(plain_rs::features::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap());
    let server = crate::server::ContentServer::start(
        dir.path(),
        &plain_libs::utils::base64::base64_encode(&[3; 32]),
        prefs,
    )
    .unwrap();
    let state = server.runtime_state().core;
    let (cert_pem, cert_key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    let listeners = crate::http::listeners::HttpListeners::start(
        plain_libs::axum::Router::new()
            .route("/health", plain_libs::axum::routing::get(crate::http::health::health)),
        0,
        0,
        cert_pem,
        cert_key,
    )
    .await
    .unwrap();
    let last_seen_at =
        (plain_libs::chrono::Utc::now() - plain_libs::chrono::Duration::minutes(2)).to_rfc3339();
    let fixture = |id: &str, port| Device {
        id: id.into(),
        name: "fixture".into(),
        ips: vec!["127.0.0.1".into()],
        port,
        device_type: plain_rs::features::chat::enums::DeviceType::Phone,
        version: "1".into(),
        platform: "test".into(),
        last_seen_at: last_seen_at.clone(),
        remote_access: None,
        signature_public_key: String::new(),
        discovery_methods: vec![
            plain_rs::features::discovery::nearby_devices::DiscoveryMethod::Lan,
        ],
    };
    for d in [fixture("alive", listeners.https_port), fixture("dead", 1)] {
        Context::from(&state).seen(d, false, true).unwrap();
    }
    Context::from(&state)
        .seen(fixture(&state.native_client_id().unwrap(), 2443), true, true)
        .unwrap();
    assert_eq!(chat_store::nearby::all(&state.db).unwrap().len(), 2);
    state
        .nearby_devices
        .scanning_modes(
            Some(true),
            None,
            chat_store::nearby::all(&state.db).unwrap(),
        )
        .unwrap();
    let before = snapshot(&state.db, &state.nearby_devices).unwrap();
    assert_eq!(before["devices"].as_array().unwrap().len(), 2);
    sweep(&state).await.unwrap();
    let after = snapshot(&state.db, &state.nearby_devices).unwrap();
    assert_eq!(after["devices"].as_array().unwrap().len(), 1);
    assert_eq!(after["devices"][0]["id"], "alive");
    assert_eq!(state.nearby_devices.snapshot().devices.len(), 1);
    assert_eq!(state.nearby_devices.snapshot().devices[0].id, "alive");
    assert_eq!(chat_store::nearby::all(&state.db).unwrap().len(), 1);
    assert!(state.nearby_devices.stale(&state.db).unwrap().is_empty());
    let mut peer = plain_rs::infrastructure::db::DPeer::new(
        "live",
        "old",
        "",
        Some(2443),
        plain_rs::features::chat::enums::DeviceType::Phone,
    );
    peer.status = plain_rs::features::chat::enums::PeerStatus::Paired;
    peer.key = plain_libs::utils::base64::base64_encode(&[72; 32]);
    chat_store::peers::save(&state.db, &[peer], chat_store::SaveMode::Insert).unwrap();
    let mut events = state.events.subscribe();
    Context::from(&state)
        .seen(fixture("live", 2443), true, true)
        .unwrap();
    let snapshot = snapshot(&state.db, &state.nearby_devices).unwrap();
    assert_eq!(
        snapshot["devices"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["id"] == "live")
            .unwrap()["status"],
        "PAIRED"
    );
    let peer = chat_store::peers::get(&state.db, "live").unwrap().unwrap();
    assert_eq!(peer.key, plain_libs::utils::base64::base64_encode(&[72; 32]));
    assert_eq!(peer.name, "fixture");
    let peer_update = events.recv().await.unwrap();
    assert_eq!(
        peer_update.event_type,
        plain_rs::features::chat::events::WS_PEER_UPDATED
    );
    assert_eq!(
        plain_libs::serde_json::from_str::<plain_libs::serde_json::Value>(&peer_update.payload).unwrap(),
        json!({"id":"live"})
    );
    let public = snapshot["devices"].as_array().unwrap().iter().find(|row| row["id"] == "live").unwrap();
    assert_eq!(public["id"], "live");
    assert_eq!(public["status"], "PAIRED");
    assert!(public.get("key").is_none());
    Context::from(&state)
        .seen(fixture("live", 2443), true, false)
        .unwrap();
    assert!(events.try_recv().is_err());
    listeners.shutdown().await;
    server.shutdown().await;
}
