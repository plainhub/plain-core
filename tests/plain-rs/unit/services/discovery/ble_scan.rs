use super::*;

fn pending(state: &CoreRuntime, owner: u64, id: &str) -> Pending {
    let scan = uuid::Uuid::new_v4();
    state.ble_scans.report_state(owner, scan, true).unwrap();
    let short_id = plain_rs::discovery::nearby_wire::short_id(id);
    let Step::Read { generation } = state.ble_scans.seen(&scan.to_string(), &short_id).unwrap()
    else {
        panic!("Expected a scheduled read");
    };
    Pending {
        scan: scan.to_string(),
        short_id,
        generation,
    }
}

#[tokio::test]
async fn inactive_and_replaced_scans_do_not_request_platform_reads() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let (owner, mut requests) = crate::test_support::HostRequests::connect(&state.host);
    state.ble_scans.set_owner(owner);
    let cancelled = pending(&state, owner, "cancelled");
    state
        .ble_scans
        .report_state(owner, cancelled.scan.parse().unwrap(), false)
        .unwrap();
    run(state.clone(), cancelled).await;
    assert!(requests.try_recv().is_err());
    let replaced = pending(&state, owner, "replaced");
    let (replacement, mut requests) = crate::test_support::HostRequests::connect(&state.host);
    run(state.clone(), replaced).await;
    assert!(requests.try_recv().is_err());
    state.ble_scans.set_owner(replacement);
    core.stop_core().await;
}

#[tokio::test]
async fn stopping_cancels_the_active_read_and_discards_the_rest_of_the_queue() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let mut state = core.runtime_state();
    state.ble_scan = std::sync::Arc::new(Runtime::default());
    let (owner, mut requests) = crate::test_support::HostRequests::connect(&state.host);
    state.ble_scans.set_owner(owner);
    state.ble_scan.schedule(pending(&state, owner, "first"));
    state.ble_scan.schedule(pending(&state, owner, "second"));
    let worker = tokio::spawn(work(state.clone()));
    let first = tokio::time::timeout(std::time::Duration::from_secs(3), requests.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first["method"], "bleScanRead");
    core.stop_core().await;
    tokio::time::timeout(std::time::Duration::from_secs(3), worker)
        .await
        .unwrap()
        .unwrap();
    assert!(state.ble_scan.queue.lock().unwrap().is_empty());
    assert!(requests.try_recv().is_err());
    assert!(state.nearby_devices.snapshot().devices.is_empty());
}

#[tokio::test]
async fn already_stopped_worker_discards_queued_reads_without_touching_the_platform() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let mut state = core.runtime_state();
    state.ble_scan = std::sync::Arc::new(Runtime::default());
    let (owner, mut requests) = crate::test_support::HostRequests::connect(&state.host);
    state.ble_scans.set_owner(owner);
    state.ble_scan.schedule(pending(&state, owner, "queued"));
    core.stop_core().await;
    work(state.clone()).await;
    assert!(state.ble_scan.queue.lock().unwrap().is_empty());
    assert!(requests.try_recv().is_err());
}

#[tokio::test]
async fn scheduled_read_supplies_canonical_request_and_checks_reply_identity() {
    for identity in ["fixture", "other"] {
        let directory = tempfile::tempdir().unwrap();
        let core = crate::test_runtime::Core::open(directory.path()).unwrap();
        let state = core.runtime_state();
        let (owner, mut requests) = crate::test_support::HostRequests::connect(&state.host);
        let scan = uuid::Uuid::new_v4();
        let short_id = plain_rs::discovery::nearby_wire::short_id("fixture");
        state.ble_scans.set_owner(owner);
        state.ble_scans.report_state(owner, scan, true).unwrap();
        let Step::Read { generation } = state.ble_scans.seen(&scan.to_string(), &short_id).unwrap()
        else {
            panic!("Expected a scheduled read");
        };
        let read = tokio::spawn(run(
            state.clone(),
            Pending {
                scan: scan.to_string(),
                short_id: short_id.clone(),
                generation,
            },
        ));
        let request = tokio::time::timeout(std::time::Duration::from_secs(3), requests.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(request["method"], "bleScanRead");
        assert_eq!(request["params"]["shortId"], "f16d05ec6b29248d");
        assert_eq!(request["params"]["scan"], scan.to_string());
        let bytes =
            plain_rs::utils::base64::base64_decode(request["params"]["request"].as_str().unwrap());
        let (metadata, body) = plain_rs::protocols::ble::parts(&bytes).unwrap();
        assert!(metadata.is_empty());
        assert_eq!(body, b"DISCOVER:");
        let payload = json!({
            "id": identity,
            "name": "Fixture",
            "port": 2443,
            "deviceType": "PHONE",
            "version": "1",
            "platform": "test",
        });
        state
            .host
            .reply(
                owner,
                json!({"id":request["id"],"result":{"payload":payload.to_string()}}),
            )
            .unwrap();
        read.await.unwrap();
        let devices = state.nearby_devices.snapshot().devices;
        if identity == "fixture" {
            assert_eq!(devices.len(), 1);
            assert_eq!(devices[0].id, identity);
        } else {
            assert!(devices.is_empty());
        }
        assert!(requests.try_recv().is_err());
        core.stop_core().await;
    }
}
