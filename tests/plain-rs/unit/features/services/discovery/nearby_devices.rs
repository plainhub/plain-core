use super::*;
use crate::features::discovery::nearby_devices::Device;
use plain_libs::serde_json::json;

#[tokio::test]
async fn nearby_rows_are_polled_without_discovery_events() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs = std::sync::Arc::new(
        crate::features::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap(),
    );
    prefs.set("client_id", "local").unwrap();
    let server = crate::test_runtime::Core::start(
        dir.path(),
        &plain_libs::utils::base64::base64_encode(&[3; 32]),
        prefs,
    )
    .unwrap();
    let state = server.runtime_state();
    let mut events = state.events.subscribe();
    let device = Device {
        id: "synthetic-nearby".into(),
        name: "Synthetic".into(),
        ips: vec![],
        port: 2443,
        device_type: crate::features::chat::enums::DeviceType::Phone,
        version: "1".into(),
        platform: "test".into(),
        last_seen_at: "2026-10-10T00:00:00.000Z".into(),
        remote_access: None,
        signature_public_key: String::new(),
        discovery_methods: vec![crate::features::discovery::nearby_devices::DiscoveryMethod::Ble],
    };
    Context::from(&state)
        .seen(device.clone(), true, false)
        .unwrap();
    let current = snapshot(&state.db, &state.nearby_devices).unwrap();
    let row = &current["devices"][0];
    assert_eq!(row["id"], device.id);
    assert_eq!(row["name"], device.name);
    assert_eq!(row["status"], "UNPAIRED");
    assert_eq!(row["bestIp"], "");
    assert_eq!(row["discoveryMethods"], json!(["BLE"]));
    assert!(row.get("eventId").is_none());
    snapshot(&state.db, &state.nearby_devices).unwrap();
    assert!(events.try_recv().is_err());
    Context::from(&state).seen(device, true, false).unwrap();
    assert!(events.try_recv().is_err());
    server.shutdown().await;
}

#[tokio::test]
async fn resident_peer_changes_emit_one_committed_notice_without_secrets_or_visibility_inference() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let server = crate::test_runtime::Core::open(directory.path()).unwrap();
    let state = server.runtime_state();
    let mut peer = crate::infrastructure::db::DPeer::new(
        "fixture-peer",
        "old",
        "192.0.2.1",
        Some(2443),
        crate::features::chat::enums::DeviceType::Phone,
    );
    peer.status = crate::features::chat::enums::PeerStatus::Paired;
    peer.key = "fixture-chat-key".into();
    peer.token = "fixture-session-token".into();
    crate::infrastructure::db::chat_store::peers::save(
        &state.db,
        &[peer],
        crate::infrastructure::db::chat_store::SaveMode::Insert,
    )
    .unwrap();
    let mut events = state.events.subscribe();
    let device = Device {
        id: "fixture-peer".into(),
        name: "new".into(),
        ips: vec!["192.0.2.2".into()],
        port: 8443,
        device_type: crate::features::chat::enums::DeviceType::Computer,
        version: "1".into(),
        platform: "test".into(),
        last_seen_at: "2026-10-10T00:00:00.000Z".into(),
        remote_access: None,
        signature_public_key: String::new(),
        discovery_methods: vec![crate::features::discovery::nearby_devices::DiscoveryMethod::Lan],
    };
    let context = Context::from(&state);
    context.seen(device.clone(), false, true).unwrap();
    let event = events.try_recv().unwrap();
    assert_eq!(
        event.event_type,
        crate::features::chat::events::WS_PEER_UPDATED
    );
    assert!(!event.is_internal());
    assert_eq!(
        plain_libs::serde_json::from_str::<plain_libs::serde_json::Value>(&event.payload).unwrap(),
        json!({"id":"fixture-peer"})
    );
    let peer = crate::infrastructure::db::chat_store::peers::get(&state.db, "fixture-peer")
        .unwrap()
        .unwrap();
    assert_eq!(peer.name, "new");
    assert_eq!(peer.ip, "192.0.2.2");
    assert_eq!(peer.port, Some(8443));
    assert_eq!(peer.key, "fixture-chat-key");
    assert!(events.try_recv().is_err());
    context.seen(device.clone(), false, true).unwrap();
    assert!(events.try_recv().is_err());
    let mut transient = device.clone();
    transient.name = "not-resident".into();
    context.seen(transient, false, false).unwrap();
    assert!(events.try_recv().is_err());
    assert_eq!(
        crate::infrastructure::db::chat_store::peers::get(&state.db, "fixture-peer")
            .unwrap()
            .unwrap()
            .name,
        "new"
    );
    server.shutdown().await;
}

#[tokio::test]
async fn bluetooth_pause_does_not_preserve_stale_lan_devices() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let server = crate::test_runtime::Core::open(directory.path()).unwrap();
    let state = server.runtime_state();
    let (generation, mut requests) = state.host.connect();
    let host = state.host.clone();
    let facts = tokio::spawn(async move {
        for paused in [true, false] {
            let request = requests.recv().await.unwrap();
            assert_eq!(request["method"], "nearbyScanFacts");
            host.reply(
                generation,
                json!({"id":request["id"],"result":{"paused":paused}}),
            )
            .unwrap();
        }
    });
    for (id, methods) in [
        (
            "expired-lan",
            vec![crate::features::discovery::nearby_devices::DiscoveryMethod::Lan],
        ),
        (
            "paused-ble",
            vec![crate::features::discovery::nearby_devices::DiscoveryMethod::Ble],
        ),
    ] {
        let device = Device {
            id: id.into(),
            name: id.into(),
            ips: vec![],
            port: 2443,
            device_type: crate::features::chat::enums::DeviceType::Phone,
            version: "1".into(),
            platform: "test".into(),
            last_seen_at: "1970-01-01T00:00:00.000Z".into(),
            remote_access: None,
        signature_public_key: String::new(),
            discovery_methods: methods,
        };
        chat_store::nearby::save(&state.db, &device.cache()).unwrap();
        state
            .nearby_devices
            .seen_at(
                device,
                std::time::Instant::now() - std::time::Duration::from_secs(61),
            )
            .unwrap();
    }
    state.nearby_devices.scanning(true, true, vec![]).unwrap();
    sweep(&state).await.unwrap();
    let devices = state.nearby_devices.snapshot().devices;
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].id, "paused-ble");
    let cached = chat_store::nearby::all(&state.db).unwrap();
    assert_eq!(cached.len(), 1);
    assert_eq!(cached[0].id, "paused-ble");
    sweep(&state).await.unwrap();
    facts.await.unwrap();
    assert!(state.nearby_devices.snapshot().devices.is_empty());
    assert!(chat_store::nearby::all(&state.db).unwrap().is_empty());
    server.shutdown().await;
}
