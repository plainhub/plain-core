#[tokio::test]
async fn nearby_snapshot_uses_the_main_contract_without_starting_discovery() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let native = core.native_graphql().unwrap();
    assert!(
        core.runtime_state()
            .public
            .sdl()
            .contains("startDiscovery: Boolean!")
    );
    let stopped = native
        .execute(async_graphql::Request::new("mutation { stopDiscovery }"))
        .await;
    assert!(stopped.errors.is_empty(), "{:?}", stopped.errors);
    assert_eq!(
        stopped.data.into_json().unwrap()["stopDiscovery"],
        serde_json::json!(true)
    );

    let selection = "{ nearbyDevices { id name ips port deviceType version platform lastSeen discoveryMethods bestIp status } }";
    let response = native.execute(async_graphql::Request::new(selection)).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["nearbyDevices"],
        serde_json::json!([])
    );
    assert!(
        !core.runtime_state().mdns.snapshot()["scanning"]
            .as_bool()
            .unwrap()
    );
    let state = core.runtime_state();
    let response = crate::graphql::executor::execute_graphql(
        &state.public,
        async_graphql::Request::new(selection),
        crate::graphql::executor::RequestContext::remote(Some(state.clone())),
    )
    .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["nearbyDevices"],
        serde_json::json!([])
    );
    core.stop_core().await;
    let response = native.execute(async_graphql::Request::new(selection)).await;
    assert!(!response.errors.is_empty());
}

#[tokio::test]
async fn is_discovering_reports_the_manual_scan_loop() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let native = core.native_graphql().unwrap();
    let selection = "{ isDiscovering }";
    let response = native.execute(async_graphql::Request::new(selection)).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["isDiscovering"],
        serde_json::json!(false),
        "a core that never started the scan loop reports no discovery"
    );
    assert_eq!(
        core.runtime_state().mdns.snapshot()["manualScanning"],
        serde_json::json!(false),
        "reading the query must not start the scan loop"
    );
    core.stop_core().await;
}

#[tokio::test]
async fn pairing_code_query_returns_a_lan_ready_model_without_starting_discovery() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let native = core.native_graphql().unwrap();
    let response = native.execute(async_graphql::Request::new("{ parsePairingCode(code: \"plainapp://pair?id=fixture&name=Device&ips=192.0.2.1&port=8443\") { id name port discoveryMethods } }")).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["parsePairingCode"],
        serde_json::json!({"id":"fixture","name":"Device","port":8443,"discoveryMethods":["LAN","QR"]})
    );
    let response = native
        .execute(async_graphql::Request::new(
            "{ parsePairingCode(code: \"ordinary text\") { id } }",
        ))
        .await;
    assert!(response.errors.is_empty());
    assert!(response.data.into_json().unwrap()["parsePairingCode"].is_null());
    assert!(!core.runtime_state().host.connected());
    core.shutdown().await;
}
