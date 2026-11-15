use super::*;

#[tokio::test]
async fn malformed_and_oversized_frames_do_not_echo_or_allocate_unbounded_payloads() {
    for bytes in [
        vec![0, 0, 0, 0],
        ((REQUEST_LIMIT + 1) as u32).to_be_bytes().to_vec(),
        vec![0, 0, 0, 5, b'{', b'o', b'o', b'p', b's'],
    ] {
        let mut input = bytes.as_slice();
        let error = read_frame::<Request>(&mut input, REQUEST_LIMIT)
            .await
            .err()
            .unwrap();
        assert!(!error.to_string().contains("oops"));
    }
    let bytes = br#"{"version":1,"command":{"type":"requests","after":null,"token":"secret"}}"#;
    assert!(plain_libs::serde_json::from_slice::<Request>(bytes).is_err());
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[tokio::test]
async fn local_listener_rejects_invalid_commands_preserves_live_sockets_and_cleans_shutdown() {
    let root = plain_libs::temporary::tempdir().unwrap();
    let core = plain_server::server::ContentServer::open(root.path()).unwrap();
    let service = Service::start(&core).await.unwrap().unwrap();
    let path = plain_libs::platform::local_socket::client_path(root.path())
        .unwrap()
        .0;
    assert!(
        requests(root.path(), None)
            .await
            .unwrap()
            .requests
            .is_empty()
    );
    assert!(Service::start(&core).await.is_err());
    assert!(path.exists());
    assert!(
        requests(root.path(), Some("invalid".into()))
            .await
            .unwrap_err()
            .to_string()
            .contains("Invalid login request id")
    );
    assert!(
        decide(
            root.path(),
            plain_libs::utils::uuid::Uuid::new_v4().into_string(),
            true
        )
        .await
        .is_err()
    );
    let mut stream = tokio::net::UnixStream::connect(&path).await.unwrap();
    write_frame(
        &mut stream,
        &Request {
            version: 9,
            command: Command::Requests { after: None },
        },
        REQUEST_LIMIT,
    )
    .await
    .unwrap();
    let reply: Reply = read_frame(&mut stream, REPLY_LIMIT).await.unwrap();
    assert!(matches!(reply.result, ReplyResult::Error { message } if message.contains("version")));
    let _unfinished = tokio::net::UnixStream::connect(&path).await.unwrap();
    core.stop_core().await;
    tokio::time::timeout(DEADLINE, service.shutdown())
        .await
        .unwrap()
        .unwrap();
    assert!(!path.parent().unwrap().exists());
    assert!(requests(root.path(), None).await.is_err());
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[tokio::test]
async fn stale_listener_is_replaced_without_affecting_another_instance() {
    use plain_libs::platform::local_socket::SocketDirectory;
    let root = plain_libs::temporary::tempdir().unwrap();
    let other_root = plain_libs::temporary::tempdir().unwrap();
    let directory = SocketDirectory::open(root.path()).unwrap();
    let path = directory.socket_path();
    let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
    let stale = directory.protect_socket().unwrap();
    drop(listener);
    let core = plain_server::server::ContentServer::open(root.path()).unwrap();
    let other = plain_server::server::ContentServer::open(other_root.path()).unwrap();
    let service = Service::start(&core).await.unwrap().unwrap();
    let other_service = Service::start(&other).await.unwrap().unwrap();
    assert!(
        requests(root.path(), None)
            .await
            .unwrap()
            .requests
            .is_empty()
    );
    core.stop_core().await;
    service.shutdown().await.unwrap();
    assert!(
        requests(other_root.path(), None)
            .await
            .unwrap()
            .requests
            .is_empty()
    );
    other.stop_core().await;
    other_service.shutdown().await.unwrap();
    drop(stale);
    drop(directory);
    assert!(!path.parent().unwrap().exists());
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[path = "management_socket/browser.rs"]
mod browser;

#[tokio::test]
async fn status_and_port_changes_follow_actual_listeners_without_stopping_the_process() {
    let root = plain_libs::temporary::tempdir().unwrap();
    let core = plain_server::server::ContentServer::open(root.path()).unwrap();
    let (cert, key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    let initial = core.start_public(0, 0, cert, key).await.unwrap();
    let listener = Service::start(&core).await.unwrap().unwrap();
    let before = status(root.path()).await.unwrap();
    assert_eq!(before.pid, std::process::id());
    assert_eq!(before.listeners, ListenerState::Running);
    assert_eq!(
        (before.http_port, before.https_port),
        (i32::from(initial.0), i32::from(initial.1))
    );
    let occupied = std::net::TcpListener::bind("0.0.0.0:0").unwrap();
    let requested = occupied.local_addr().unwrap().port();
    let cli = crate::cli::Cli::try_parse_from([
        "plainapp",
        "prefs",
        "http_port",
        &requested.to_string(),
        "--data-dir",
        root.path().to_str().unwrap(),
    ])
    .unwrap();
    crate::cli::prefs::run(&cli.paths().unwrap(), "http_port", &requested.to_string())
        .await
        .unwrap();
    let after = status(root.path()).await.unwrap();
    assert_eq!(after.pid, before.pid);
    assert_eq!(after.listeners, ListenerState::Running);
    assert_ne!(after.http_port, i32::from(requested));
    let actual = core.http_ports().await.unwrap();
    assert_eq!(
        (after.http_port, after.https_port),
        (i32::from(actual.0), i32::from(actual.1))
    );
    assert_eq!(
        core.runtime().prefs.get::<i32>("http_port").unwrap(),
        Some(after.http_port)
    );
    let client = plain_libs::protocols::http_client::builder()
        .no_proxy()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();
    for (scheme, port) in [("http", after.http_port), ("https", after.https_port)] {
        assert!(
            client
                .get(format!("{scheme}://127.0.0.1:{port}/health"))
                .send()
                .await
                .unwrap()
                .status()
                .is_success()
        );
    }
    set_pref(
        root.path(),
        "https_port".into(),
        plain_libs::serde_json::json!(requested),
    )
    .await
    .unwrap();
    let after_https = status(root.path()).await.unwrap();
    assert_eq!(after_https.pid, before.pid);
    assert_ne!(after_https.https_port, 0);
    assert_ne!(after_https.https_port, i32::from(requested));
    let actual_https = core.http_ports().await.unwrap().1;
    assert_eq!(after_https.https_port, i32::from(actual_https));
    assert!(
        requests(root.path(), None)
            .await
            .unwrap()
            .requests
            .is_empty()
    );
    core.stop_public().await;
    let stopped = status(root.path()).await.unwrap();
    assert_eq!(stopped.listeners, ListenerState::Stopped);
    assert_eq!((stopped.http_port, stopped.https_port), (0, 0));
    core.stop_core().await;
    listener.shutdown().await.unwrap();
    assert!(not_running(&status(root.path()).await.unwrap_err()));
}

#[tokio::test]
async fn management_requests_preserve_existing_key_permissions_and_frame_bounds() {
    let root = plain_libs::temporary::tempdir().unwrap();
    let core = plain_server::server::ContentServer::open(root.path()).unwrap();
    let listener = Service::start(&core).await.unwrap().unwrap();
    let before = core.runtime().prefs.entries();
    for key in [
        "client_id",
        "adb_token",
        "master_secret",
        "signature_key_pair",
        "unknown_key",
    ] {
        assert!(
            set_pref(
                root.path(),
                key.into(),
                plain_libs::serde_json::json!("replacement")
            )
            .await
            .is_err()
        );
        assert_eq!(core.runtime().prefs.entries(), before);
    }
    let large = "x".repeat(65537);
    assert!(
        set_pref(
            root.path(),
            "device_name".into(),
            plain_libs::serde_json::json!(large)
        )
        .await
        .is_err()
    );
    assert_eq!(core.runtime().prefs.entries(), before);
    let unknown = br#"{"version":1,"command":{"type":"status","action":"run"}}"#;
    assert!(plain_libs::serde_json::from_slice::<Request>(unknown).is_err());
    core.stop_core().await;
    listener.shutdown().await.unwrap();
}
