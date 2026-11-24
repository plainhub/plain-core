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
    assert!(nearby_type.contains("lastSeen: Instant!"));
    assert!(!nearby_type.contains("deviceType: String!"));
    let stopped = native
        .execute(plain_graphql::Request::new("mutation { stopDiscovery }"))
        .await;
    assert!(stopped.errors.is_empty(), "{:?}", stopped.errors);
    assert_eq!(
        stopped.data.into_json().unwrap()["stopDiscovery"],
        plain_libs::serde_json::json!(true)
    );

    let selection = "{ nearbyDevices { id name ips port deviceType version platform lastSeen discoveryMethods bestIp status } }";
    let response = native.execute(plain_graphql::Request::new(selection)).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["nearbyDevices"],
        plain_libs::serde_json::json!([])
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
        plain_libs::serde_json::json!([])
    );
    core.stop_core().await;
    let response = native.execute(plain_graphql::Request::new(selection)).await;
    assert!(!response.errors.is_empty());
}

#[tokio::test]
async fn nearby_snapshot_selects_recent_endpoints_and_restores_discovery_facts() {
    use plain_rs::features::discovery::nearby_devices::{Device, DiscoveryMethod};
    use plain_rs::infrastructure::db::chat_store::nearby;

    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let now = plain_libs::chrono::DateTime::parse_from_rfc3339(&plain_rs::infrastructure::db::now_iso())
        .unwrap()
        .with_timezone(&plain_libs::chrono::Utc);
    let mut live_only = Vec::new();
    for (id, ip, port, hours_ago, remote_access) in [
        ("old-id", "192.0.2.1", 8443, 4, None),
        ("latest-id", "192.0.2.1", 8443, 1, Some(true)),
        ("other-port", "192.0.2.1", 9443, 3, Some(false)),
        ("other-ip", "192.0.2.2", 8443, 2, None),
        ("expired", "192.0.2.3", 8443, 25, Some(true)),
        ("boundary", "192.0.2.4", 8443, 24, None),
        ("ble-first", "", 8443, 5, None),
        ("ble-second", "", 8443, 6, None),
    ] {
        let device = Device {
            id: id.into(),
            name: "Fixture".into(),
            ips: if ip.is_empty() { vec![] } else { vec![ip.into()] },
            port,
            remote_access,
            signature_public_key: format!("{id}-public-key"),
            device_type: plain_rs::features::chat::enums::DeviceType::Computer,
            version: "1".into(),
            platform: "test".into(),
            last_seen_at: (now - plain_libs::chrono::Duration::hours(hours_ago)).to_rfc3339(),
            discovery_methods: vec![if ip.is_empty() { DiscoveryMethod::Ble } else { DiscoveryMethod::Lan }],
        };
        if ip.is_empty() || id == "expired" {
            live_only.push(device.clone());
        }
        if !ip.is_empty() {
            nearby::save(&state.db, &device.cache()).unwrap();
        }
    }
    let recent = nearby::recent(&state.db, now).unwrap();
    assert!(!recent.iter().any(|row| row.id == "expired"));
    assert!(recent.iter().any(|row| row.id == "boundary"));
    let later = nearby::recent(&state.db, now + plain_libs::chrono::Duration::milliseconds(1)).unwrap();
    assert!(!later.iter().any(|row| row.id == "boundary"));
    state.nearby_devices.scanning(true, false, later).unwrap();
    let old_probe = state.nearby_devices.stale(&state.db).unwrap()
        .into_iter().find(|probe| probe.device.id == "old-id").unwrap();
    assert!(state.nearby_devices.verified(&state.db, &old_probe, true).unwrap());
    assert_eq!(nearby::all(&state.db).unwrap().iter().find(|row| row.id == "old-id").unwrap().last_seen_at, old_probe.device.last_seen_at);
    for device in live_only {
        state.nearby_devices.observe(device).unwrap();
    }
    let response = core.native_graphql().unwrap().execute(plain_graphql::Request::new(
        "{ nearbyDevices { id bestIp port remoteAccess signaturePublicKey } }",
    )).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["nearbyDevices"], plain_libs::serde_json::json!([
        {"id":"latest-id","bestIp":"192.0.2.1","port":8443,"remoteAccess":true,"signaturePublicKey":"latest-id-public-key"},
        {"id":"other-ip","bestIp":"192.0.2.2","port":8443,"remoteAccess":null,"signaturePublicKey":"other-ip-public-key"},
        {"id":"other-port","bestIp":"192.0.2.1","port":9443,"remoteAccess":false,"signaturePublicKey":"other-port-public-key"},
        {"id":"ble-first","bestIp":"","port":8443,"remoteAccess":null,"signaturePublicKey":"ble-first-public-key"},
        {"id":"ble-second","bestIp":"","port":8443,"remoteAccess":null,"signaturePublicKey":"ble-second-public-key"},
    ]));
    core.shutdown().await;
}

#[tokio::test]
async fn nearby_snapshot_keeps_one_row_for_a_shared_endpoint_when_preferred_addresses_differ() {
    use plain_rs::features::discovery::nearby_devices::{Device, DiscoveryMethod};

    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let native = core.native_graphql().unwrap();
    let now = plain_libs::chrono::Utc::now();
    let alternate = Device {
        id: "previous-id".into(),
        name: "Fixture".into(),
        ips: vec!["2001:db8::1".into()],
        port: 8443,
        remote_access: None,
        signature_public_key: "previous-key".into(),
        device_type: plain_rs::features::chat::enums::DeviceType::Computer,
        version: "1".into(),
        platform: "test".into(),
        last_seen_at: (now - plain_libs::chrono::Duration::minutes(1)).to_rfc3339(),
        discovery_methods: vec![DiscoveryMethod::Lan],
    };
    let current = Device {
        id: "current-id".into(),
        ips: vec!["2001:db8::2".into(), "2001:db8::1".into()],
        signature_public_key: "current-key".into(),
        last_seen_at: now.to_rfc3339(),
        ..alternate.clone()
    };
    for device in [
        current,
        Device {
            id: "other-port".into(),
            port: 9443,
            ..alternate.clone()
        },
        Device {
            id: "other-address".into(),
            ips: vec!["2001:db8::3".into()],
            ..alternate.clone()
        },
    ] {
        state.nearby_devices.observe(device).unwrap();
    }
    for alternate in [None, Some(alternate)] {
        if let Some(device) = alternate {
            state.nearby_devices.observe(device).unwrap();
        }
        let response = native
            .execute(plain_graphql::Request::new("{ nearbyDevices { id bestIp } }"))
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert_eq!(
            response.data.into_json().unwrap()["nearbyDevices"],
            plain_libs::serde_json::json!([
                {"id":"current-id","bestIp":"2001:db8::2"},
                {"id":"other-address","bestIp":"2001:db8::3"},
                {"id":"other-port","bestIp":"2001:db8::1"},
            ])
        );
    }
    core.shutdown().await;
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
        plain_libs::serde_json::json!(false),
        "a core that never started the scan loop reports no discovery"
    );
    assert_eq!(
        core.runtime_state().mdns.snapshot()["manualScanning"],
        plain_libs::serde_json::json!(false),
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
    let encoded = plain_libs::serde_json::to_string(&display.payload).unwrap();
    let response = native.execute(plain_graphql::Request::new(format!(
        "{{ parsePairingQrCode(code: {encoded}) {{ id name port deviceType discoveryMethods }} }}"
    ))).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["parsePairingQrCode"],
        plain_libs::serde_json::json!({"id":"fixture","name":"Device","port":8443,"deviceType":"COMPUTER","discoveryMethods":["LAN","QR"]})
    );
    let response = native
        .execute(plain_graphql::Request::new(
            "{ parsePairingQrCode(code: \"ordinary text\") { id } }",
        ))
        .await;
    assert!(response.errors.is_empty());
    assert!(response.data.into_json().unwrap()["parsePairingQrCode"].is_null());
    assert!(!core.runtime_state().host.connected());
    core.shutdown().await;
}
