use super::*;
use crate::features::chat::enums::DeviceType;
use crate::features::services::peers::management::Identity;
use crate::infrastructure::db::DPeer;
use crate::infrastructure::db::chat_store::SaveMode;
use crate::infrastructure::db::chat_store::peers;
use crate::runtime::Runtime as CoreRuntime;
use crate::test_runtime::Core as ContentServer;
use plain_libs::serde_json::json;
use tokio::sync::watch;

async fn fixture() -> (
    plain_libs::temporary::TempDir,
    ContentServer,
    CoreRuntime,
    DPeer,
    Identity,
) {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = ContentServer::open_with_cache(
        &directory.path().join("data"),
        &directory.path().join("cache"),
    )
    .unwrap();
    let state = core.runtime_state();
    state.management.shutdown().await;
    let mut peer = DPeer::new(
        "peer",
        "Original",
        "127.0.0.1",
        Some(18443),
        DeviceType::Computer,
    );
    peer.token = plain_libs::utils::base64::base64_encode(&[7; 32]);
    peer.key = "pairing-key".into();
    peer.public_key = "signature-key".into();
    peers::save(&state.db, &[peer.clone()], SaveMode::Insert).unwrap();
    let identity = Identity {
        url: "wss://127.0.0.1:18443/?cid=fixture".into(),
        token: peer.token.clone(),
        ip: peer.ip.clone(),
        port: peer.port,
    };
    (directory, core, state, peer, identity)
}

#[tokio::test]
async fn observed_names_update_only_the_current_peer_and_identical_or_empty_names_do_not_write() {
    let (_directory, core, state, before, identity) = fixture().await;
    let (_cancel, canceled) = watch::channel(false);
    let mut events = state.events.subscribe();
    state.db.with_conn(|connection| {
        let writes = connection.total_changes();
        assert!(
            apply(
                connection, &state, &before.id, &identity, &canceled, "Original"
            )
            .unwrap()
        );
        assert!(apply(connection, &state, &before.id, &identity, &canceled, "").unwrap());
        assert_eq!(connection.total_changes(), writes);
        assert!(
            apply(
                connection, &state, &before.id, &identity, &canceled, "Renamed"
            )
            .unwrap()
        );
        assert_eq!(connection.total_changes(), writes + 1);
        assert!(
            apply(
                connection, &state, &before.id, &identity, &canceled, "Renamed"
            )
            .unwrap()
        );
        assert_eq!(connection.total_changes(), writes + 1);
    });
    let after = peers::get(&state.db, &before.id).unwrap().unwrap();
    assert_eq!(after.name, "Renamed");
    assert_eq!(
        (
            after.key,
            after.token,
            after.public_key,
            after.ip,
            after.port,
            after.status,
            after.created_at
        ),
        (
            before.key,
            before.token,
            before.public_key,
            before.ip,
            before.port,
            before.status,
            before.created_at
        )
    );
    let event = events.try_recv().unwrap();
    assert_eq!(
        event.event_type,
        crate::features::chat::events::WS_PEER_UPDATED
    );
    assert_eq!(
        plain_libs::serde_json::from_str::<plain_libs::serde_json::Value>(&event.payload).unwrap(),
        json!({"id":"peer"})
    );
    assert!(events.try_recv().is_err());
    core.stop_core().await;
}

#[tokio::test]
async fn cancellation_credentials_and_address_changes_reject_stale_observations_without_writes() {
    let (_directory, core, state, peer, identity) = fixture().await;
    let (cancel, canceled) = watch::channel(false);
    let mut events = state.events.subscribe();
    for field in ["token", "ip", "port", "cancel"] {
        let mut old = identity.clone();
        match field {
            "token" => old.token = plain_libs::utils::base64::base64_encode(&[8; 32]),
            "ip" => old.ip = "127.0.0.2".into(),
            "port" => old.port = Some(19443),
            _ => {
                cancel.send_replace(true);
            }
        }
        state.db.with_conn(|connection| {
            let before = connection.total_changes();
            assert!(!apply(connection, &state, &peer.id, &old, &canceled, "Stale").unwrap());
            assert_eq!(connection.total_changes(), before);
        });
        cancel.send_replace(false);
    }
    assert_eq!(
        peers::get(&state.db, &peer.id).unwrap().unwrap().name,
        "Original"
    );
    assert!(events.try_recv().is_err());
    core.stop_core().await;
    state.db.with_conn(|connection| {
        assert!(
            !apply(
                connection, &state, &peer.id, &identity, &canceled, "Stopped"
            )
            .unwrap()
        )
    });
}
