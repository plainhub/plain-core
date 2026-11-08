#![cfg(feature = "http_transport")]

use plain_rs::features::services::peers::peer_transport::*;
use plain_rs::infrastructure::db::DPeer;
use plain_rs::infrastructure::db::chat_store::peers;
use plain_rs::infrastructure::transport::router::TransportType;
use plain_rs::runtime::Runtime as ServerState;
use serde_json::Value;
use serde_json::json;
use std::sync::Arc;

fn peer() -> DPeer {
    let mut p = DPeer::new(
        "fixture",
        "fixture",
        "",
        Some(443),
        plain_rs::features::chat::enums::DeviceType::Phone,
    );
    p.key = plain_libs::utils::base64::base64_encode(&[7; 32]);
    p
}
fn state() -> (plain_libs::temporary::TempDir, crate::server::ContentServer, ServerState) {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs =
        Arc::new(plain_rs::features::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    prefs.set("client_id", "actor").unwrap();
    let server = crate::server::ContentServer::start(
        dir.path(),
        &plain_libs::utils::base64::base64_encode(&[1; 32]),
        prefs,
    )
    .unwrap();
    let state = server.runtime_state().core;
    peers::save(
        &state.db,
        &[peer()],
        plain_rs::infrastructure::db::chat_store::SaveMode::Insert,
    )
    .unwrap();
    (dir, server, state)
}

#[tokio::test]
async fn aware_config_uses_current_pairing_identity_and_rejects_corrupt_keys() {
    let (_dir, server, state) = state();
    state.prefs.set("https_port", 2443).unwrap();
    let config =
        aware_config_for_peer(&state, &peers::get(&state.db, "fixture").unwrap().unwrap()).unwrap();
    assert!(config.is_client);
    assert_eq!(config.local_port, 2443);
    assert_eq!(
        config.pmk.unwrap(),
        plain_libs::utils::base64::base64_encode(&[7; 32])
    );
    let native = server.native_graphql().unwrap();
    assert_eq!(snapshot(&state)["active"], json!([]));
    let removed = native
        .execute(plain_graphql::Request::new(
            "{ __type(name: \"PeerAwareConfig\") { name } }",
        ))
        .await;
    assert!(removed.errors.is_empty(), "{:?}", removed.errors);
    assert!(removed.data.into_json().unwrap()["__type"].is_null());
    let mut corrupt = peers::get(&state.db, "fixture").unwrap().unwrap();
    corrupt.key = "%%%".into();
    peers::save(
        &state.db,
        &[corrupt],
        plain_rs::infrastructure::db::chat_store::SaveMode::Update,
    )
    .unwrap();
    assert!(
        aware_config_for_peer(&state, &peers::get(&state.db, "fixture").unwrap().unwrap()).is_err()
    );
    server.shutdown().await;
}

#[tokio::test]
async fn host_transport_snapshots_follow_current_routes_and_reconnect() {
    let _discovery = plain_rs::features::services::discovery::mdns_runtime::TEST_LOCK
        .lock()
        .await;
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state().core;
    let mut first = peer();
    first.id = "b-peer".into();
    let ticket = state
        .transport
        .begin(&first, &[TransportType::Ble])
        .unwrap()
        .ticket
        .unwrap();
    let mut second = first.clone();
    second.id = "a-peer".into();
    state
        .transport
        .begin(&second, &[TransportType::Aware])
        .unwrap();
    let bridge = core.host_events().unwrap();
    assert_eq!(
        transport_callback(&bridge).await,
        json!([
            {"peerId":"a-peer","transport":"AWARE"}, {"peerId":"b-peer","transport":"BLE"},
        ])
    );
    state.transport.abort(&ticket);
    notify(&state);
    assert_eq!(
        transport_callback(&bridge).await,
        json!([{"peerId":"a-peer","transport":"AWARE"}])
    );
    state.transport.forget("a-peer");
    let replacement = core.host_events().unwrap();
    assert_eq!(transport_callback(&replacement).await, json!([]));
    core.stop_core().await;
}

async fn transport_callback(bridge: &crate::host::events::HostEvents) -> Value {
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let request = bridge.next_request().await.unwrap();
            bridge
                .reply(json!({"id":request["id"],"result":null}))
                .unwrap();
            if request["method"] == "peerTransportsChanged" {
                break request["params"]["active"].clone();
            }
        }
    })
    .await
    .unwrap()
}
