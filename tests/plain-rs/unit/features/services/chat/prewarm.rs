use super::*;

#[test]
fn start_receipt_requires_actual_boolean_and_no_errors() {
    assert!(!started(&json!({"data":{"startAware":false}})).unwrap());
    assert!(started(&json!({"data":{"startAware":true},"errors":[]})).unwrap());
    assert!(started(&json!({"data":{"startAware":true},"errors":[{"message":"failed"}]})).is_err());
    assert!(started(&json!({"data":{}})).is_err());
}

#[tokio::test]
async fn scan_reads_aware_flags_from_discover_and_checks_full_advertised_identity() {
    let id = "706c6169-2d74-8e53-9a16-f08b62c591d3";
    for reply_id in [id, "706c6169-b903-841f-a6ce-32d75908f4ab"] {
        let directory = plain_libs::temporary::tempdir().unwrap();
        let core = crate::test_runtime::Core::open(directory.path()).unwrap();
        let state = core.runtime_state();
        let (owner, mut requests) = crate::test_support::HostRequests::connect(&state.host);
        let scan_state = state.clone();
        let task = tokio::spawn(async move { Native(&scan_state).discover(id).await });
        let scan = tokio::time::timeout(std::time::Duration::from_secs(3), requests.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(scan["method"], "peerTransportPrewarmScan");
        assert_eq!(scan["params"]["clientId"], id);
        state
            .host
            .reply(owner, json!({"id":scan["id"],"result":id}))
            .unwrap();
        let read = tokio::time::timeout(std::time::Duration::from_secs(3), requests.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(read["method"], "bleScanRead");
        assert_eq!(read["params"]["clientId"], id);
        let bytes =
            plain_libs::utils::base64::base64_decode(read["params"]["request"].as_str().unwrap());
        assert_eq!(
            crate::infrastructure::protocols::ble::nearby_body(&bytes).unwrap(),
            b"DISCOVER:"
        );
        let payload = json!({"id":reply_id,"name":"Device","port":2443,"deviceType":"PHONE","version":"1","platform":"test","awareSupported":true,"awareRunning":false});
        state
            .host
            .reply(
                owner,
                json!({"id":read["id"],"result":{"payload":payload.to_string()}}),
            )
            .unwrap();
        let result = task.await.unwrap();
        if reply_id == id {
            let found = result.unwrap().unwrap();
            assert_eq!(found.client_id, id);
            assert!(found.aware_supported);
            assert!(!found.aware_running);
        } else {
            assert!(result.is_err());
        }
        assert!(requests.try_recv().is_err());
        core.stop_core().await;
    }
}
