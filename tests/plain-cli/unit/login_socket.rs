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
    assert!(serde_json::from_slice::<Request>(bytes).is_err());
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[tokio::test]
async fn local_listener_rejects_invalid_commands_preserves_live_sockets_and_cleans_shutdown() {
    let root = tempfile::tempdir().unwrap();
    let core = plain_server::server::ContentServer::open(root.path()).unwrap();
    let service = Service::start(&core).await.unwrap().unwrap();
    let path = plain_rs::platform::local_socket::client_path(root.path())
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
        decide(root.path(), uuid::Uuid::new_v4().to_string(), true)
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
    use plain_rs::platform::local_socket::SocketDirectory;
    let root = tempfile::tempdir().unwrap();
    let other_root = tempfile::tempdir().unwrap();
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
#[path = "login_socket/browser.rs"]
mod browser;
