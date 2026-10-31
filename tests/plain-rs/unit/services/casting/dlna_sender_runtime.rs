use super::*;
use serde_json::json;

#[test]
fn command_rejects_missing_fields_and_unknown_actions() {
    for body in [
        json!({"action":"select"}),
        json!({"action":"seek"}),
        json!({"action":"cast","item":{"path":"a"}}),
        json!({"action":"snapshot","nativeFallback":true}),
        json!({"action":"unknown"}),
    ] {
        assert!(serde_json::from_value::<Command>(body).is_err());
    }
}

#[tokio::test]
async fn multicast_lease_is_reused_only_within_the_same_host_generation() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let (generation, mut requests) = state.host.connect();
    let acquiring = {
        let state = state.clone();
        tokio::spawn(async move { acquire_permission(&state, &state.cast.receiver_lease).await })
    };
    let first = requests.recv().await.unwrap();
    assert_eq!(first["method"], "mdnsMulticast");
    assert_eq!(first["params"]["acquire"], true);
    state
        .host
        .reply(generation, json!({"id":first["id"],"result":true}))
        .unwrap();
    acquiring.await.unwrap().unwrap();
    acquire_permission(&state, &state.cast.receiver_lease)
        .await
        .unwrap();
    assert!(requests.try_recv().is_err());

    let (generation, mut requests) = state.host.connect();
    let acquiring = {
        let state = state.clone();
        tokio::spawn(async move { acquire_permission(&state, &state.cast.receiver_lease).await })
    };
    let release = requests.recv().await.unwrap();
    assert_eq!(release["params"]["lease"], first["params"]["lease"]);
    assert_eq!(release["params"]["acquire"], false);
    state
        .host
        .reply(generation, json!({"id":release["id"],"result":false}))
        .unwrap();
    let second = requests.recv().await.unwrap();
    assert_eq!(second["params"]["acquire"], true);
    assert_ne!(second["params"]["lease"], first["params"]["lease"]);
    state
        .host
        .reply(generation, json!({"id":second["id"],"result":true}))
        .unwrap();
    acquiring.await.unwrap().unwrap();
    assert_eq!(
        state
            .cast
            .receiver_lease
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .generation,
        Some(generation)
    );
    state.cast.receiver_lease.lock().unwrap().take();
    core.stop_core().await;
}

#[tokio::test]
async fn sender_updates_deliver_versioned_host_snapshots_without_internal_session_fields() {
    let dir = tempfile::tempdir().unwrap();
    let server = crate::test_runtime::Core::open(dir.path()).unwrap();
    let state = server.runtime_state();
    let (generation, mut requests) = state.host.connect();
    {
        let mut snapshot = state.cast.snapshot.lock().unwrap();
        snapshot.sid = "private-session".into();
        snapshot.active = true;
        snapshot.current_uri = "fixture".into();
    }
    state.cast.publish(&state);
    let request = requests.recv().await.unwrap();
    assert_eq!(request["method"], "dlnaSenderChanged");
    assert_eq!(request["params"]["currentUri"], "fixture");
    assert_eq!(request["params"]["version"], 1);
    assert!(request["params"].get("sid").is_none());
    assert!(request["params"].get("active").is_none());
    state
        .host
        .reply(generation, json!({"id":request["id"],"result":null}))
        .unwrap();
    state.host.disconnect(generation);
    server.stop_core().await;
}

#[tokio::test]
async fn saturated_host_receives_latest_sender_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let server = crate::test_runtime::Core::open(dir.path()).unwrap();
    let state = server.runtime_state();
    let (generation, mut requests) = state.host.connect();
    for _ in 0..plain_rs::host::bridge::REQUEST_CAPACITY {
        state.host.notify("blocked", json!({})).unwrap();
    }
    state.cast.snapshot.lock().unwrap().current_uri = "first".into();
    state.cast.publish(&state);
    state.cast.snapshot.lock().unwrap().current_uri = "latest".into();
    state.cast.publish(&state);
    for _ in 0..plain_rs::host::bridge::REQUEST_CAPACITY {
        let request = requests.recv().await.unwrap();
        state
            .host
            .reply(generation, json!({"id":request["id"],"result":null}))
            .unwrap();
    }
    let recovery = tokio::time::timeout(std::time::Duration::from_secs(3), requests.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(recovery["method"], "dlnaSenderChanged");
    assert_eq!(recovery["params"]["currentUri"], "latest");
    assert_eq!(recovery["params"]["version"], 2);
    state
        .host
        .reply(generation, json!({"id":recovery["id"],"result":null}))
        .unwrap();
    state.host.disconnect(generation);
    server.stop_core().await;
}
