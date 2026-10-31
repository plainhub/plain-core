use plain_rs::services::casting::dlna_sender_runtime::*;
use plain_rs::services::casting::sender::types::DiscoveredDevice;
use serde_json::json;
use std::sync::Arc;
use std::sync::Mutex;

#[tokio::test]
async fn queue_mutations_preserve_duplicates_and_order_through_the_main_schema() {
    let dir = tempfile::tempdir().unwrap();
    let server = crate::server::ContentServer::open(dir.path()).unwrap();
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

#[tokio::test]
async fn sender_controls_use_soap_and_release_the_original_subscription() {
    use axum::{Router, body::to_bytes, extract::Request, routing::any};
    use plain_rs::services::casting::sender::types::UpnpService;
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
    let server = crate::server::ContentServer::start(
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
