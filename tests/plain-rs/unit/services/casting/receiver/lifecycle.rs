use super::*;
use std::sync::atomic::Ordering;

#[tokio::test]
async fn listener_port_requires_an_active_public_server() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let state = core.runtime_state();
    state.http_server.started((45123, 45124));
    assert_eq!(active_port(&state), None);
    state
        .peer_status
        .public_active
        .store(true, Ordering::SeqCst);
    assert_eq!(active_port(&state), Some(45123));
    state.http_server.stopped();
    assert_eq!(active_port(&state), None);
    state
        .peer_status
        .public_active
        .store(false, Ordering::SeqCst);
    core.stop_core().await;
}

#[tokio::test]
async fn enabling_without_http_defers_platform_work_and_retry() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let (_, mut requests) = state.host.connect();
    state.prefs.set("dlna", true).unwrap();
    reconcile(&state).await.unwrap();
    assert!(!state.dlna.is_running());
    assert!(retry_locked(&state).await.is_err());
    assert!(!state.dlna.snapshot().await.is_retrying);
    assert!(requests.try_recv().is_err());
    assert!(plain_rs::prefs::dlna::enabled(&state.prefs));
    core.stop_core().await;
}

#[tokio::test]
async fn receiver_without_samba_still_requires_multicast_permission() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let host = state.host.clone();
    let (generation, mut requests) = crate::test_support::HostRequests::connect(&host);
    let responder = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let result = match request["method"].as_str().unwrap() {
                "systemCapabilities" => serde_json::json!([]),
                "mdnsMulticast" => serde_json::json!(false),
                other => panic!("Unexpected platform primitive: {other}"),
            };
            host.reply(
                generation,
                serde_json::json!({"id": request["id"], "result": result}),
            )
            .unwrap();
        }
    });
    let error = start_locked(&state, 45123).await.unwrap_err();
    assert_eq!(error, "Multicast permission unavailable");
    assert_eq!(state.dlna.snapshot().await.start_error, error);
    assert!(!state.dlna.is_running());
    assert!(state.cast.receiver_lease.lock().unwrap().is_none());
    state.host.disconnect(generation);
    core.stop_core().await;
    responder.await.unwrap();
}
