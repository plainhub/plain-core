use plain_rs::db::chat_store;
use plain_rs::discovery::nearby_devices::Device;
use plain_rs::services::discovery::nearby_devices::*;
use serde_json::json;

#[tokio::test]
async fn rust_sweep_honors_radio_pause_and_real_tls_verdicts() {
    use std::sync::Arc;
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap());
    prefs.set("client_id", "local").unwrap();
    let server = crate::server::ContentServer::start(
        dir.path(),
        &plain_rs::utils::base64::base64_encode(&[3; 32]),
        prefs,
    )
    .unwrap();
    let state = server.runtime_state().core;
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let listeners = crate::http::listeners::HttpListeners::start(
        axum::Router::new().route("/nearby", axum::routing::post(|| async { "1" })),
        0,
        0,
        cert.cert.pem().into_bytes(),
        cert.key_pair.serialize_pem().into_bytes(),
    )
    .await
    .unwrap();
    let fixture = |id: &str, port| Device {
        id: id.into(),
        name: "fixture".into(),
        ips: vec!["127.0.0.1".into()],
        port,
        device_type: "PHONE".into(),
        version: "1".into(),
        platform: "test".into(),
        last_seen_at: "1970-01-01T00:00:00.000Z".into(),
        discovery_methods: vec!["LAN".into()],
    };
    for d in [fixture("alive", listeners.https_port), fixture("dead", 1)] {
        Context::from(&state).seen(d, false, true).unwrap();
    }
    Context::from(&state)
        .seen(fixture("local", 2443), true, true)
        .unwrap();
    assert_eq!(chat_store::nearby::all(&state.db).unwrap().len(), 2);
    state
        .nearby_devices
        .scanning_modes(
            None,
            Some(true),
            chat_store::nearby::all(&state.db).unwrap(),
        )
        .unwrap();
    let (generation, mut requests) = state.host.connect();
    let host = state.host.clone();
    let facts = tokio::spawn(async move {
        for paused in [true, false] {
            let request = requests.recv().await.unwrap();
            assert_eq!(request["method"], "nearbyScanFacts");
            host.reply(
                generation,
                json!({"id":request["id"],"result":{"paused":paused,"interfaces":[]}}),
            )
            .unwrap();
        }
    });
    sweep(&state).await.unwrap();
    assert_eq!(state.nearby_devices.snapshot().devices.len(), 2);
    sweep(&state).await.unwrap();
    facts.await.unwrap();
    assert_eq!(state.nearby_devices.snapshot().devices.len(), 1);
    assert_eq!(state.nearby_devices.snapshot().devices[0].id, "alive");
    assert_eq!(chat_store::nearby::all(&state.db).unwrap().len(), 1);
    assert!(state.nearby_devices.stale(&state.db).unwrap().is_empty());
    let mut peer = plain_rs::db::DPeer::new(
        "live",
        "old",
        "",
        Some(2443),
        plain_rs::chat::enums::DeviceType::Phone,
    );
    peer.status = plain_rs::chat::enums::PeerStatus::Paired;
    peer.key = "preserved-key".into();
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
    assert_eq!(peer.key, "preserved-key");
    assert_eq!(peer.name, "fixture");
    let peer_update = events.recv().await.unwrap();
    assert_eq!(
        peer_update.event_type,
        plain_rs::chat::events::WS_PEER_UPDATED
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&peer_update.payload).unwrap(),
        json!({"id":"live"})
    );
    let invalidation = events.recv().await.unwrap();
    assert_eq!(invalidation.event_type, EVENT_UPDATED);
    let first = events.recv().await.unwrap();
    assert_eq!(
        first.event_type,
        plain_rs::chat::events::WS_NEARBY_DEVICE_FOUND
    );
    let public: serde_json::Value = serde_json::from_str(&first.payload).unwrap();
    assert_eq!(public["id"], "live");
    assert_eq!(public["status"], "PAIRED");
    assert!(public.get("key").is_none());
    Context::from(&state)
        .seen(fixture("live", 2443), true, false)
        .unwrap();
    let next = events.recv().await.unwrap();
    assert_eq!(next.event_type, EVENT_UPDATED);
    assert!(events.try_recv().is_err());
    listeners.shutdown().await;
    server.shutdown().await;
}
