use plain_libs::dlna::types::DiscoveredDevice;
use plain_rs::features::services::casting::dlna_sender_runtime::*;
use serde_json::json;
use std::sync::Arc;
use std::sync::Mutex;

#[tokio::test]
async fn queue_mutations_preserve_duplicates_and_order_through_the_main_schema() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let server = crate::server::ContentServer::open(dir.path()).unwrap();
    let native = server.native_graphql().unwrap();
    for path in ["a.mp3", "b.mp3", "a.mp3"] {
        let response = native.execute(plain_graphql::Request::new("mutation($path:String!){addDlnaItem(item:{path:$path,title:$path}){items{path}}}").variables(plain_graphql::Variables::from_json(json!({"path":path})))).await;
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
    use plain_libs::dlna::types::UpnpService;
    let calls = Arc::new(Mutex::new(Vec::<(String, String, String)>::new()));
    let captured = calls.clone();
    let stop_seen = Arc::new(tokio::sync::Notify::new());
    let stop_release = Arc::new(tokio::sync::Notify::new());
    let stopped = stop_seen.clone();
    let release = stop_release.clone();
    let router = Router::new().fallback(any(move |request: Request| {
        let captured = captured.clone();
        let stopped = stopped.clone();
        let release = release.clone();
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
            if action.ends_with("#Stop\"") {
                stopped.notify_one();
                release.notified().await;
            }
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
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs =
        Arc::new(plain_rs::features::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    let server = crate::server::ContentServer::start(
        dir.path(),
        &plain_libs::utils::base64::base64_encode(&[3; 32]),
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
        host_address: "127.0.0.1".into(),
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
    let old_uri = state.cast.transport.lock().unwrap().uri.clone();
    let old_name = old_uri.rsplit('/').next().unwrap();
    let peer = "127.0.0.1".parse().unwrap();
    assert!(
        state.cast.media.lookup(old_name, peer).is_some(),
        "Exit retains media access"
    );
    let fields = std::collections::HashMap::from([("TransportState".into(), "STOPPED".into())]);
    let before = calls.lock().unwrap().len();
    plain_rs::features::services::casting::cast_runtime::apply_poll(&state, &fields)
        .await
        .unwrap();
    assert!(
        state.cast.media.lookup(old_name, peer).is_some(),
        "A transient stop does not revoke"
    );
    plain_rs::features::services::casting::cast_runtime::apply_poll(&state, &fields)
        .await
        .unwrap();
    assert_eq!(state.cast.snapshot().current_uri, "/next.mp4");
    assert!(state.cast.media.lookup(old_name, peer).is_none());
    assert_eq!(
        calls.lock().unwrap().len(),
        before + 2,
        "Track advance sets a fresh URI and starts playback"
    );
    plain_rs::features::services::casting::cast_runtime::apply_poll(&state, &fields)
        .await
        .unwrap();
    plain_rs::features::services::casting::cast_runtime::apply_poll(&state, &fields)
        .await
        .unwrap();
    assert_eq!(
        state.cast.snapshot().current_uri,
        "/video.mp4",
        "Cast queue wraps"
    );
    state.cast.snapshot.lock().unwrap().items.clear();
    let completed_uri = state.cast.transport.lock().unwrap().uri.clone();
    let completed_name = completed_uri.rsplit('/').next().unwrap();
    plain_rs::features::services::casting::cast_runtime::apply_poll(&state, &fields)
        .await
        .unwrap();
    plain_rs::features::services::casting::cast_runtime::apply_poll(&state, &fields)
        .await
        .unwrap();
    assert!(!state.cast.media.active());
    assert_eq!(state.cast.snapshot().current_uri, "/video.mp4");
    execute(&state, &state.cast, Command::Play).await.unwrap();
    let replay_uri = state.cast.transport.lock().unwrap().uri.clone();
    assert_ne!(replay_uri, completed_uri);
    assert!(state.cast.media.lookup(completed_name, peer).is_none());
    assert!(
        state
            .cast
            .media
            .lookup(replay_uri.rsplit('/').next().unwrap(), peer)
            .is_some()
    );
    let active_uri = state.cast.transport.lock().unwrap().uri.clone();
    let active_name = active_uri.rsplit('/').next().unwrap();
    let stream = state.cast.media.lookup(active_name, peer).unwrap();
    let stopping = {
        let state = state.clone();
        tokio::spawn(async move { execute(&state, &state.cast, Command::Stop).await })
    };
    tokio::time::timeout(std::time::Duration::from_secs(5), stop_seen.notified())
        .await
        .unwrap();
    assert!(
        state.cast.media.lookup(active_name, peer).is_none(),
        "Revoke before remote Stop responds"
    );
    stream.cancelled().await;
    stop_release.notify_one();
    stopping.await.unwrap().unwrap();
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
async fn stop_during_pending_poll_does_not_advance_the_queue() {
    use axum::{Router, extract::Request, routing::any};
    use plain_libs::dlna::types::UpnpService;
    use std::time::Duration;
    use tokio::sync::Notify;

    let calls = Arc::new(Mutex::new(Vec::<String>::new()));
    let poll_seen = Arc::new(Notify::new());
    let poll_release = Arc::new(Notify::new());
    let captured = calls.clone();
    let seen = poll_seen.clone();
    let release = poll_release.clone();
    let router = Router::new().fallback(any(move |request: Request| {
        let captured = captured.clone();
        let seen = seen.clone();
        let release = release.clone();
        async move {
            let action = request
                .headers()
                .get("soapaction")
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
                .to_owned();
            captured.lock().unwrap().push(action.clone());
            if action.ends_with("#GetPositionInfo\"") {
                seen.notify_one();
                release.notified().await;
            }
            let body = if action.ends_with("#GetTransportInfo\"") {
                "<Envelope><Body><GetTransportInfoResponse><CurrentTransportState>STOPPED</CurrentTransportState></GetTransportInfoResponse></Body></Envelope>"
            } else {
                "<Envelope><Body/></Envelope>"
            };
            (
                [("SID", "uuid:poll-subscription"), ("TIMEOUT", "Second-3600")],
                body,
            )
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let renderer = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs =
        Arc::new(plain_rs::features::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    let server = crate::server::ContentServer::start(
        dir.path(),
        &plain_libs::utils::base64::base64_encode(&[3; 32]),
        prefs,
    )
    .unwrap();
    let state = server.runtime_state();
    state.prefs.set("background_service_enabled", true).unwrap();
    let device = DiscoveredDevice {
        udn: "uuid:poll-renderer".into(),
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
        id: "uuid:poll-renderer".into(),
        host_address: "127.0.0.1".into(),
        ..Default::default()
    });
    let current = Item {
        path: "/current.mp4".into(),
        ..Default::default()
    };
    state.cast.snapshot.lock().unwrap().items = vec![
        current.clone(),
        Item {
            path: "/next.mp4".into(),
            ..Default::default()
        },
    ];
    run(&state, Command::Cast { item: current }).await.unwrap();
    assert!(state.cast.transport.lock().unwrap().started);
    state.cast.transport.lock().unwrap().stopped = true;
    let uri = state.cast.transport.lock().unwrap().uri.clone();
    let name = uri.rsplit('/').next().unwrap();
    let peer = "127.0.0.1".parse().unwrap();
    let stream = state.cast.media.lookup(name, peer).unwrap();

    tokio::time::timeout(Duration::from_secs(5), poll_seen.notified())
        .await
        .unwrap();
    assert!(state.cast.operations.try_lock().is_err());
    let stopping = {
        let state = state.clone();
        tokio::spawn(async move { run(&state, Command::Stop).await })
    };
    tokio::time::timeout(Duration::from_secs(5), stream.cancelled())
        .await
        .unwrap();
    assert!(state.cast.media.lookup(name, peer).is_none());
    assert!(!stopping.is_finished());
    assert!(
        !calls
            .lock()
            .unwrap()
            .iter()
            .any(|action| action.ends_with("#Stop\"")),
        "Stop waits for the poll's operation lock after revoking media"
    );
    poll_release.notify_one();
    tokio::time::timeout(Duration::from_secs(5), stopping)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let actions = calls.lock().unwrap().clone();
    assert!(
        actions
            .iter()
            .any(|action| action.ends_with("#GetTransportInfo\""))
    );
    assert!(actions.iter().any(|action| action.ends_with("#Stop\"")));
    for action in ["#SetAVTransportURI\"", "#Play\""] {
        assert_eq!(
            actions
                .iter()
                .filter(|value| value.ends_with(action))
                .count(),
            1,
            "The stale STOPPED poll must not start the next queued item: {actions:?}"
        );
    }
    assert!(state.cast.snapshot().current_uri.is_empty());
    assert!(state.cast.snapshot().current_device.is_none());
    server.shutdown().await;
    renderer.abort();
}
