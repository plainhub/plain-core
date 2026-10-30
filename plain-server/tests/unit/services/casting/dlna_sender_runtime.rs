use super::*;
#[tokio::test]
async fn queue_mutations_preserve_duplicates_and_order_through_the_main_schema() {
    let dir = tempfile::tempdir().unwrap();
    let server = crate::assembly::server::ContentServer::open(dir.path()).unwrap();
    let native = server.native_graphql().unwrap();
    for path in ["a.mp3", "b.mp3", "a.mp3"] {
        let response = native.execute(async_graphql::Request::new("mutation($path:String!){addDlnaItem(item:{path:$path,title:$path}){items{path}}}").variables(async_graphql::Variables::from_json(json!({"path":path})))).await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
    }
    let response = native
        .execute("mutation {reorderDlnaItems(from:0,to:1){items{path}}}".into())
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let row = response.data.into_json().unwrap();
    assert_eq!(
        row["reorderDlnaItems"]["items"].as_array().unwrap().len(),
        3
    );
    assert_eq!(row["reorderDlnaItems"]["items"][0]["path"], "b.mp3");
    let response = native
        .execute("mutation {removeDlnaItemAt(index:0){items{path}}}".into())
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let row = response.data.into_json().unwrap();
    assert_eq!(
        row["removeDlnaItemAt"]["items"].as_array().unwrap().len(),
        2
    );
    assert_eq!(row["removeDlnaItemAt"]["items"][0]["path"], "a.mp3");
    let response = native
        .execute("mutation {clearDlnaItems{items{path}}}".into())
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert!(
        response.data.into_json().unwrap()["clearDlnaItems"]["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    server.shutdown().await;
}
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
async fn sender_controls_use_soap_and_release_the_original_subscription() {
    use crate::services::casting::sender::types::UpnpService;
    use axum::{Router, body::to_bytes, extract::Request, routing::any};
    let calls = Arc::new(Mutex::new(Vec::<(String, String, String)>::new()));
    let captured = calls.clone();
    let router = Router::new().fallback(any(move |request: Request| {
        let captured = captured.clone();
        async move {
            let method = request.method().as_str().to_owned();
            let action = request
                .headers()
                .get("soapaction")
                .and_then(|s| s.to_str().ok())
                .unwrap_or_default()
                .to_owned();
            let sid = request
                .headers()
                .get("sid")
                .and_then(|s| s.to_str().ok())
                .unwrap_or_default()
                .to_owned();
            let body = to_bytes(request.into_body(), 1024 * 1024).await.unwrap();
            captured.lock().unwrap().push((
                method.clone(),
                if action.is_empty() { sid } else { action },
                String::from_utf8(body.to_vec()).unwrap(),
            ));
            (
                [
                    ("SID", "uuid:test-subscription"),
                    ("TIMEOUT", "Second-3600"),
                ],
                "<Envelope><Body/></Envelope>",
            )
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let renderer = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    let server = crate::assembly::server::ContentServer::start(
        dir.path(),
        &plain_rs::utils::base64::base64_encode(&[3; 32]),
        prefs,
    )
    .unwrap();
    let state = server.runtime_state();
    let host = state.host.clone();
    let (_generation, mut requests) = crate::test_support::HostRequests::connect(&host);
    let device = DiscoveredDevice {
        udn: "uuid:renderer".into(),
        location: format!("{base}/description.xml"),
        has_av_transport: true,
        av_transport: UpnpService {
            service_type: "urn:schemas-upnp-org:service:AVTransport:1".into(),
            control_url: format!("{base}/control"),
            event_sub_url: format!("{base}/event"),
            ..Default::default()
        },
        ..Default::default()
    };
    state
        .cast
        .devices
        .lock()
        .unwrap()
        .insert(device.udn.clone(), device);
    state.cast.snapshot.lock().unwrap().current_device = Some(Device {
        id: "uuid:renderer".into(),
        ..Default::default()
    });
    execute(
        &state,
        &state.cast,
        Command::Cast {
            item: Item {
                path: "/video.mp4".into(),
                title: "A & B".into(),
                ..Default::default()
            },
        },
    )
    .await
    .unwrap();
    assert!(
        state.cast.snapshot().items.is_empty(),
        "Tap to cast must not enqueue"
    );
    assert_eq!(state.cast.snapshot().sid, "uuid:test-subscription");
    execute(&state, &state.cast, Command::Pause).await.unwrap();
    assert!(!state.cast.snapshot().playing);
    execute(
        &state,
        &state.cast,
        Command::Seek {
            position_ms: 3723999,
        },
    )
    .await
    .unwrap();
    execute(&state, &state.cast, Command::Exit).await.unwrap();
    assert_eq!(state.cast.snapshot().current_uri, "/video.mp4");
    assert!(state.cast.snapshot().current_device.is_some());
    assert!(state.cast.snapshot().sid.is_empty());
    let rows = calls.lock().unwrap().clone();
    assert!(rows[0].1.contains("SetAVTransportURI"));
    assert!(rows[0].2.contains("A &amp;amp; B"));
    assert!(rows[1].1.contains("#Play"));
    assert_eq!(rows[2].0, "SUBSCRIBE");
    assert!(
        rows.iter()
            .any(|row| row.1.contains("#Seek") && row.2.contains("01:02:03"))
    );
    assert!(
        rows.iter()
            .any(|row| row.0 == "UNSUBSCRIBE" && row.1 == "uuid:test-subscription")
    );
    assert!(
        !rows.iter().any(|row| row.1.contains("#Stop")),
        "Exit retains receiver playback"
    );
    state.prefs.set("background_service_enabled", true).unwrap();
    state.cast.snapshot.lock().unwrap().items = vec![
        Item {
            path: "/video.mp4".into(),
            ..Default::default()
        },
        Item {
            path: "/next.mp4".into(),
            title: "Next".into(),
            ..Default::default()
        },
    ];
    let notify = |sequence: u32, sid: Option<&str>| {
        let mut builder = axum::http::Request::builder()
            .method("NOTIFY")
            .header("SEQ", sequence.to_string());
        if let Some(sid) = sid {
            builder = builder.header("SID", sid);
        }
        builder.body(axum::body::Body::from(r#"<Event><InstanceID val="0"><TransportState val="STOPPED"/></InstanceID></Event>"#)).unwrap()
    };
    let callback = crate::http::dlna::callback;
    let before = calls.lock().unwrap().len();
    callback(axum::extract::State(state.clone()), notify(10, None)).await;
    assert_eq!(state.cast.snapshot().current_uri, "/next.mp4");
    assert_eq!(
        calls.lock().unwrap().len(),
        before + 1,
        "Track advance sets URI without explicit Play"
    );
    callback(axum::extract::State(state.clone()), notify(10, None)).await;
    callback(axum::extract::State(state.clone()), notify(9, None)).await;
    callback(
        axum::extract::State(state.clone()),
        notify(11, Some("uuid:foreign")),
    )
    .await;
    assert_eq!(
        state.cast.snapshot().current_uri,
        "/next.mp4",
        "Duplicate, stale and foreign notifications must not advance"
    );
    callback(axum::extract::State(state.clone()), notify(11, None)).await;
    assert_eq!(
        state.cast.snapshot().current_uri,
        "/video.mp4",
        "Cast queue wraps"
    );
    execute(&state, &state.cast, Command::Stop).await.unwrap();
    assert!(state.cast.snapshot().current_device.is_none());
    assert!(state.cast.snapshot().current_uri.is_empty());
    assert!(
        calls
            .lock()
            .unwrap()
            .iter()
            .any(|row| row.1.contains("#Stop"))
    );
    assert!(requests.try_recv().is_err());
    server.shutdown().await;
    renderer.abort();
}

#[tokio::test]
async fn multicast_lease_is_reused_only_within_the_same_host_generation() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::assembly::server::ContentServer::open(directory.path()).unwrap();
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
    let server = crate::assembly::server::ContentServer::open(dir.path()).unwrap();
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
    state.host.reply(generation, json!({"id":request["id"],"result":null})).unwrap();
    state.host.disconnect(generation);
    server.stop_core().await;
}

#[tokio::test]
async fn saturated_host_receives_latest_sender_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let server = crate::assembly::server::ContentServer::open(dir.path()).unwrap();
    let state = server.runtime_state();
    let (generation, mut requests) = state.host.connect();
    for _ in 0..crate::host::bridge::REQUEST_CAPACITY {
        state.host.notify("blocked", json!({})).unwrap();
    }
    state.cast.snapshot.lock().unwrap().current_uri = "first".into();
    state.cast.publish(&state);
    state.cast.snapshot.lock().unwrap().current_uri = "latest".into();
    state.cast.publish(&state);
    for _ in 0..crate::host::bridge::REQUEST_CAPACITY {
        let request = requests.recv().await.unwrap();
        state.host.reply(generation, json!({"id":request["id"],"result":null})).unwrap();
    }
    let recovery = tokio::time::timeout(std::time::Duration::from_secs(3), requests.recv()).await.unwrap().unwrap();
    assert_eq!(recovery["method"], "dlnaSenderChanged");
    assert_eq!(recovery["params"]["currentUri"], "latest");
    assert_eq!(recovery["params"]["version"], 2);
    state.host.reply(generation, json!({"id":recovery["id"],"result":null})).unwrap();
    state.host.disconnect(generation);
    server.stop_core().await;
}
