#[tokio::test]
async fn nearby_snapshot_uses_the_main_contract_without_starting_discovery() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let native = core.native_graphql().unwrap();
    assert!(
        core.runtime_state()
            .public
            .sdl()
            .contains("startDiscovery: Boolean!")
    );
    let sdl = core.runtime_state().public.sdl();
    let nearby_type = sdl
        .split("type NearbyDevice {")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
    assert!(nearby_type.contains("deviceType: DeviceType!"));
    assert!(!nearby_type.contains("deviceType: String!"));
    let stopped = native
        .execute(plain_graphql::Request::new("mutation { stopDiscovery }"))
        .await;
    assert!(stopped.errors.is_empty(), "{:?}", stopped.errors);
    assert_eq!(
        stopped.data.into_json().unwrap()["stopDiscovery"],
        serde_json::json!(true)
    );

    let selection = "{ nearbyDevices { id name ips port deviceType version platform lastSeen discoveryMethods bestIp status } }";
    let response = native.execute(plain_graphql::Request::new(selection)).await;
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
        plain_graphql::Request::new(selection),
        crate::graphql::executor::RequestContext::remote(Some(state.clone())),
    )
    .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["nearbyDevices"],
        serde_json::json!([])
    );
    core.stop_core().await;
    let response = native.execute(plain_graphql::Request::new(selection)).await;
    assert!(!response.errors.is_empty());
}

#[tokio::test]
async fn is_discovering_reports_the_manual_scan_loop() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let native = core.native_graphql().unwrap();
    let selection = "{ isDiscovering }";
    let response = native.execute(plain_graphql::Request::new(selection)).await;
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
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let native = core.native_graphql().unwrap();
    let display = plain_rs::features::services::discovery::pairing_qr::create_display(
        &core.runtime_state().core,
        "fixture".into(),
        plain_rs::features::services::discovery::pairing::Device {
            name: "Device".into(), port: 8443,
            device_type: plain_rs::features::chat::enums::DeviceType::Computer,
            ips: vec!["192.0.2.1".into()], aware_supported: false,
        },
    ).unwrap();
    let encoded = serde_json::to_string(&display.payload).unwrap();
    let response = native.execute(plain_graphql::Request::new(format!(
        "{{ parsePairingCode(code: {encoded}) {{ id name port deviceType discoveryMethods }} }}"
    ))).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["parsePairingCode"],
        serde_json::json!({"id":"fixture","name":"Device","port":8443,"deviceType":"COMPUTER","discoveryMethods":["LAN","QR"]})
    );
    let response = native
        .execute(plain_graphql::Request::new(
            "{ parsePairingCode(code: \"ordinary text\") { id } }",
        ))
        .await;
    assert!(response.errors.is_empty());
    assert!(response.data.into_json().unwrap()["parsePairingCode"].is_null());
    assert!(!core.runtime_state().host.connected());
    core.shutdown().await;
}
