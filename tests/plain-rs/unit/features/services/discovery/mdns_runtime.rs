use super::*;
use serde_json::json;

#[tokio::test(start_paused = true)]
async fn network_change_burst_restarts_once_and_accepts_the_next_burst() {
    use std::sync::atomic::AtomicUsize;
    let runtime = Runtime::default();
    let (_stop, stopped) = tokio::sync::watch::channel(false);
    let calls = Arc::new(AtomicUsize::new(0));
    let restart = || {
        let calls = calls.clone();
        async move {
            calls.fetch_add(1, Ordering::SeqCst);
        }
    };
    assert!(runtime.schedule_network_restart(stopped.clone(), restart()));
    tokio::task::yield_now().await;
    tokio::time::advance(std::time::Duration::from_millis(799)).await;
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(!runtime.schedule_network_restart(stopped.clone(), restart()));
    tokio::time::advance(std::time::Duration::from_millis(1)).await;
    tokio::task::yield_now().await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(runtime.schedule_network_restart(stopped, restart()));
    tokio::task::yield_now().await;
    tokio::time::advance(std::time::Duration::from_millis(800)).await;
    tokio::task::yield_now().await;
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test(start_paused = true)]
async fn network_restart_cancels_on_runtime_stop_and_explicit_close() {
    use std::sync::atomic::AtomicUsize;
    for close in [false, true] {
        let runtime = Runtime::default();
        let (stop, stopped) = tokio::sync::watch::channel(false);
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        assert!(
            runtime.schedule_network_restart(stopped.clone(), async move {
                count.fetch_add(1, Ordering::SeqCst);
            })
        );
        tokio::task::yield_now().await;
        if close {
            runtime.cancel_network_restart();
        } else {
            stop.send_replace(true);
        }
        tokio::time::advance(std::time::Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        stop.send_replace(true);
        assert!(!runtime.schedule_network_restart(stopped, async { panic!("stopped runtime") }));
    }
}

#[tokio::test(start_paused = true)]
async fn stopping_a_network_restart_cancels_its_in_progress_work() {
    use std::sync::atomic::AtomicUsize;
    let runtime = Runtime::default();
    let (stop, stopped) = tokio::sync::watch::channel(false);
    let (started, running) = tokio::sync::oneshot::channel();
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    assert!(runtime.schedule_network_restart(stopped, async move {
        started.send(()).unwrap();
        std::future::pending::<()>().await;
        count.fetch_add(1, Ordering::SeqCst);
    }));
    tokio::task::yield_now().await;
    tokio::time::advance(std::time::Duration::from_millis(800)).await;
    running.await.unwrap();
    stop.send_replace(true);
    tokio::task::yield_now().await;
    assert!(
        runtime
            .network_restart
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .is_finished()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test(start_paused = true)]
async fn receiver_starts_without_http_and_retries_when_platform_permissions_change() {
    use std::sync::atomic::{AtomicBool, AtomicUsize};
    let _runtime = TEST_LOCK.lock().await;
    let directory = tempfile::tempdir().unwrap();
    let prefs = Arc::new(
        crate::features::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap(),
    );
    prefs.set("client_id", "fixture-resident-receiver").unwrap();
    prefs
        .set("mdns_hostname", "fixture-resident-receiver.local")
        .unwrap();
    let server = crate::test_runtime::Core::start(
        directory.path(),
        &plain_libs::utils::base64::base64_encode(&[8; 32]),
        prefs,
    )
    .unwrap();
    let state = server.runtime_state();
    let (generation, mut requests) = crate::test_support::HostRequests::connect(&state.host);
    let granted = Arc::new(AtomicBool::new(false));
    let calls = Arc::new(AtomicUsize::new(0));
    let host = state.host.clone();
    let permission = granted.clone();
    let sampled = calls.clone();
    let responder = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let result = match request["method"].as_str().unwrap() {
                "mdnsMulticast" => {
                    if request["params"]["acquire"] == true {
                        sampled.fetch_add(1, Ordering::SeqCst);
                    }
                    json!(
                        request["params"]["acquire"] == false || permission.load(Ordering::SeqCst)
                    )
                }
                method => panic!("unexpected host method {method}"),
            };
            host.reply(generation, json!({"id":request["id"],"result":result}))
                .unwrap();
        }
    });
    Runtime::host_connected(state.clone(), generation.wrapping_sub(1)).await;
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    Runtime::host_connected(state.clone(), generation).await;
    assert_eq!(state.mdns.snapshot()["receiver"], false);
    granted.store(true, Ordering::SeqCst);
    let mut events = state.events.subscribe();
    state.mdns.permissions_changed(state.clone());
    tokio::task::yield_now().await;
    tokio::time::advance(std::time::Duration::from_millis(800)).await;
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while events.recv().await.unwrap().event_type != WS_MDNS_UPDATED {}
    })
    .await
    .unwrap();
    let snapshot = state.mdns.snapshot();
    assert_eq!(snapshot["receiver"], true);
    assert_eq!(snapshot["published"], false);
    assert_eq!(snapshot["scanning"], false);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    state.mdns.permissions_changed(state.clone());
    tokio::time::advance(std::time::Duration::from_secs(1)).await;
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    server.shutdown().await;
    assert!(OWNER.lock().await.is_none());
    responder.abort();
}

#[tokio::test]
async fn discovery_requests_multicast_permission_without_querying_capabilities() {
    let _runtime = TEST_LOCK.lock().await;
    let directory = tempfile::tempdir().unwrap();
    let token = plain_libs::utils::base64::base64_encode(&[17; 32]);
    let prefs = Arc::new(
        crate::features::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap(),
    );
    prefs
        .set("client_id", "fixture-discovery-permission")
        .unwrap();
    prefs
        .set("mdns_hostname", "fixture-discovery-permission.local")
        .unwrap();
    let server = crate::test_runtime::Core::start(directory.path(), &token, prefs).unwrap();
    let state = server.runtime_state();
    let (generation, mut incoming) = crate::test_support::HostRequests::connect(&state.host);
    let worker = tokio::spawn({
        let state = state.clone();
        async move { control(&state, Request::Publish {}).await }
    });
    let request = incoming.recv().await.unwrap();
    assert_eq!(request["method"], "mdnsMulticast");
    state
        .host
        .reply(generation, json!({"id":request["id"],"result":false}))
        .unwrap();
    assert!(worker.await.unwrap().is_err());
    assert!(incoming.try_recv().is_err());
    assert_eq!(state.mdns.snapshot()["receiver"], false);
    state.host.disconnect(generation);
    server.shutdown().await;
}

#[tokio::test]
async fn inactive_runtime_cleanup_and_updates_do_not_request_platform_capabilities() {
    use futures_util::FutureExt;
    let directory = tempfile::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let (generation, mut requests) = crate::test_support::HostRequests::connect(&state.host);
    for request in [
        Request::Update {},
        Request::Restart {},
        Request::Stop {},
        Request::Unpublish {},
    ] {
        state
            .mdns
            .execute(&state, request)
            .now_or_never()
            .expect("Inactive runtime commands must complete without a host request")
            .unwrap();
    }
    assert!(requests.try_recv().is_err());
    state.host.disconnect(generation);
    core.stop_core().await;
}

#[tokio::test]
async fn refused_or_cancelled_multicast_grants_leave_the_next_runtime_available() {
    let _runtime = TEST_LOCK.lock().await;
    for cancelled in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let core = crate::test_runtime::Core::open(directory.path()).unwrap();
        let state = core.runtime_state();
        state.prefs.set("client_id", "fixture-mdns-grant").unwrap();
        state.prefs.set("mdns_hostname", "fixture-mdns-grant.local").unwrap();
        let first = Arc::new(Runtime::default());
        let second = Arc::new(Runtime::default());
        let (generation, mut requests) = state.host.connect();
        let pending = tokio::spawn({
            let first = first.clone();
            let state = state.clone();
            async move { first.initialize(&state).await }
        });
        let request = tokio::time::timeout(std::time::Duration::from_secs(2), requests.recv()).await.unwrap().unwrap();
        assert_eq!(request["method"], "mdnsMulticast");
        assert_eq!(request["params"]["acquire"], true);
        if cancelled {
            pending.abort();
            assert!(pending.await.unwrap_err().is_cancelled());
        } else {
            state.host.reply(generation, json!({"id":request["id"],"result":false})).unwrap();
            assert!(pending.await.unwrap().is_err());
        }
        let host = state.host.clone();
        let responder = tokio::spawn(async move {
            while let Some(request) = requests.recv().await {
                assert_eq!(request["method"], "mdnsMulticast");
                host.reply(generation, json!({"id":request["id"],"result":true})).unwrap();
            }
        });
        let result = second.initialize(&state).await;
        first.close(&state.host).await;
        second.close(&state.host).await;
        state.host.disconnect(generation);
        responder.await.unwrap();
        core.stop_core().await;
        assert!(result.is_ok(), "A rejected or cancelled grant must not own the global runtime: {result:?}");
    }
}
