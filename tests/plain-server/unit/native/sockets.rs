use super::*;
use crate::{server::ContentServer, test_support::HostRequests};
use plain_rs::infrastructure::events::WsEvent;
use serde_json::json;

fn fixture() -> (plain_libs::temporary::TempDir, ContentServer) {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let core = ContentServer::open_with_cache(&dir.path().join("data"), &dir.path().join("cache"))
        .unwrap();
    (dir, core)
}
async fn read(session: &SocketSession) -> Vec<u8> {
    tokio::time::timeout(Duration::from_secs(3), session.read())
        .await
        .unwrap()
        .unwrap()
}
async fn removed(sockets: &NativeSockets, handle: &str) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while sockets.session(handle).is_ok() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
fn fake_host(
    core: &ContentServer,
    capable: bool,
) -> (
    tokio::task::JoinHandle<()>,
    mpsc::UnboundedReceiver<serde_json::Value>,
) {
    let host = core.runtime_state().host.clone();
    let (generation, mut requests) = HostRequests::connect(&host);
    let (sent, received) = mpsc::unbounded_channel();
    let task = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let result = match request["method"].as_str().unwrap() {
                "mdnsMulticast" => json!(true),
                "systemCapabilities" => {
                    if capable {
                        json!(["SCREEN_MIRROR"])
                    } else {
                        json!([])
                    }
                }
                "systemScreenMirrorControls" | "systemScreenMirrorResetTouch" => {
                    sent.send(request.clone()).unwrap();
                    json!(true)
                }
                method => panic!("Unexpected fake host request: {method}"),
            };
            host.reply(generation, json!({"id": request["id"], "result": result}))
                .unwrap();
        }
    });
    (task, received)
}

#[tokio::test]
async fn native_editor_shares_main_schema_updates_without_a_listener_and_owner_close_wakes_read() {
    let (_dir, core) = fixture();
    assert_eq!(core.port, 0);
    let sockets = core.native_sockets().unwrap();
    let scope = NativeRequestScope::default();
    let handle = sockets
        .open(SocketKind::ImageEditor, None, scope.clone())
        .unwrap();
    let session = sockets.session(&handle).unwrap();
    assert_eq!(session.scope_id(), scope.id());
    assert_eq!(read(&session).await, [0]);
    assert_eq!(read(&session).await, [3]);
    let state = core.runtime_state();
    state.prefs.set("remote_access", false).unwrap();
    state
        .main_ws
        .close_client(&state.subscription_sessions, None);
    let response = core
        .native_graphql()
        .unwrap()
        .execute(plain_graphql::Request::new(
            "mutation { broadcastImageEditorUpdate(id: \"fixture\", update: \"AP8B\") }",
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(read(&session).await, b"\x04\x07fixture\x00\xff\x01");
    assert_eq!(
        session.send(vec![1]).await.unwrap_err(),
        "Socket is read-only"
    );
    let mut pending = Box::pin(session.read());
    assert!(futures_util::poll!(pending.as_mut()).is_pending());
    assert_eq!(
        session.read().await.unwrap_err(),
        "Concurrent socket reader"
    );
    scope.close();
    assert!(pending.await.is_err());
    removed(&sockets, &handle).await;
    core.stop_core().await;
}

#[tokio::test]
async fn native_screen_filters_and_orders_binary_and_cleans_up_only_the_last_shared_connection() {
    let (_dir, core) = fixture();
    let (host, mut effects) = fake_host(&core, true);
    let state = core.runtime_state();
    let sockets = core.native_sockets().unwrap();
    let first_scope = NativeRequestScope::default();
    let second_scope = NativeRequestScope::default();
    let first = sockets
        .open(SocketKind::Screen, None, first_scope.clone())
        .unwrap();
    let second = sockets
        .open(SocketKind::Screen, None, second_scope.clone())
        .unwrap();
    let first_session = sockets.session(&first).unwrap();
    let second_session = sockets.session(&second).unwrap();
    assert_eq!(read(&first_session).await, [0]);
    assert_eq!(read(&second_session).await, [0]);
    for event in [
        WsEvent::broadcast_binary("IMAGE_EDITOR_UPDATE", vec![9]),
        WsEvent::targeted("SCREEN_MIRROR_VIDEO", "wrong".into(), "other"),
    ] {
        state.main_ws.screen_events.send(event).unwrap();
    }
    state
        .main_ws
        .screen_events
        .send(WsEvent::broadcast_binary(
            "SCREEN_MIRROR_VIDEO",
            vec![0, 255, 1],
        ))
        .unwrap();
    state
        .main_ws
        .screen_events
        .send(WsEvent::broadcast_binary(
            "SCREEN_MIRROR_AUDIO",
            vec![2, 0, 3],
        ))
        .unwrap();
    for session in [&first_session, &second_session] {
        assert_eq!(read(session).await, [1, 0, 255, 1]);
        assert_eq!(read(session).await, [2, 2, 0, 3]);
    }
    first_session
        .send(br#"{"type":"screenMirrorControl","input":{"action":"BACK"}}"#.to_vec())
        .await
        .unwrap();
    first_session
        .send(vec![0x54, 1, 0, 0, 0, 7, 0xff, 0xff, 0, 0, 0, 0])
        .await
        .unwrap();
    assert_eq!(
        effects.recv().await.unwrap()["params"]["inputs"][0]["action"],
        "BACK"
    );
    assert_eq!(
        effects.recv().await.unwrap()["params"]["inputs"][0]["action"],
        "TOUCH_DOWN"
    );
    assert!(
        first_session
            .send(vec![0; MAX_SOCKET_CONTROL_BYTES + 1])
            .await
            .is_err()
    );
    first_scope.close();
    removed(&sockets, &first).await;
    assert!(effects.try_recv().is_err());
    second_scope.close();
    removed(&sockets, &second).await;
    assert_eq!(
        effects.recv().await.unwrap()["method"],
        "systemScreenMirrorResetTouch"
    );
    assert!(effects.try_recv().is_err());
    core.stop_core().await;
    host.abort();
}

#[tokio::test]
async fn unsupported_screen_and_invalid_peer_do_not_publish_readiness_or_touch_the_host() {
    let (_dir, core) = fixture();
    let (host, mut effects) = fake_host(&core, false);
    let sockets = core.native_sockets().unwrap();
    for peer in [
        None,
        Some("https://untrusted.invalid".into()),
        Some("unknown-peer".into()),
    ] {
        let handle = sockets
            .open(SocketKind::Screen, peer, NativeRequestScope::default())
            .unwrap();
        let session = sockets.session(&handle).unwrap();
        assert!(
            tokio::time::timeout(Duration::from_secs(3), session.read())
                .await
                .unwrap()
                .is_err()
        );
        removed(&sockets, &handle).await;
    }
    assert!(effects.try_recv().is_err());
    core.stop_core().await;
    host.abort();
}

#[tokio::test]
async fn stalled_native_reader_fails_closed_and_releases_its_owner() {
    let (_dir, core) = fixture();
    let sockets = core.native_sockets().unwrap();
    let handle = sockets
        .open(SocketKind::ImageEditor, None, NativeRequestScope::default())
        .unwrap();
    let session = sockets.session(&handle).unwrap();
    assert_eq!(read(&session).await, [0]);
    assert_eq!(read(&session).await, [3]);
    for n in 0..=QUEUE_CAPACITY {
        core.runtime_state()
            .events
            .send(WsEvent::broadcast_binary(
                "IMAGE_EDITOR_UPDATE",
                vec![n as u8],
            ))
            .unwrap();
    }
    removed(&sockets, &handle).await;
    assert!(session.read().await.unwrap_err().contains("backpressure"));
    core.stop_core().await;
}

#[test]
fn binary_frame_and_queue_budgets_fail_before_delivery_and_release_on_drop() {
    assert!(
        SocketKind::Screen
            .frame("IMAGE_EDITOR_UPDATE", &[])
            .is_err()
    );
    assert!(
        SocketKind::ImageEditor
            .frame("IMAGE_EDITOR_READY", &[1])
            .is_err()
    );
    assert!(
        SocketKind::Screen
            .frame("SCREEN_MIRROR_VIDEO", &vec![0; MAX_SOCKET_FRAME_BYTES])
            .is_err()
    );
    let (frames, mut incoming) = mpsc::channel(1);
    let budget = Arc::new(tokio::sync::Semaphore::new(3));
    let output = Output {
        frames,
        bytes: budget.clone(),
    };
    publish(&output, vec![1, 2, 3]).unwrap();
    assert!(publish(&output, vec![4]).is_err());
    assert_eq!(budget.available_permits(), 0);
    drop(incoming.try_recv().unwrap());
    assert_eq!(budget.available_permits(), 3);
}

#[tokio::test]
async fn closed_scope_and_stopped_runtime_never_open_a_session() {
    let (_dir, core) = fixture();
    let sockets = core.native_sockets().unwrap();
    let scope = NativeRequestScope::default();
    scope.close();
    assert!(sockets.open(SocketKind::ImageEditor, None, scope).is_err());
    core.stop_core().await;
    assert!(
        sockets
            .open(SocketKind::ImageEditor, None, NativeRequestScope::default())
            .is_err()
    );
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn paired_screen_uses_pinned_transport_for_binary_controls_and_releases_remote_touch() {
    use plain_rs::{
        features::{
            chat::enums::{DeviceType, PeerStatus},
            services::sessions,
        },
        infrastructure::db::{
            DPeer,
            chat_store::{SaveMode, peers},
        },
    };
    let (_local_dir, local) = fixture();
    let (_remote_dir, remote) = fixture();
    local.runtime_state().management.shutdown().await;
    remote.runtime_state().management.shutdown().await;
    let (host, mut effects) = fake_host(&remote, true);
    let (cert, key) = plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    let (_, port) = remote.start_public(0, 0, cert, key).await.unwrap();
    let client_id = local.native_graphql().unwrap().client_id().to_owned();
    let peer_id = remote.native_graphql().unwrap().client_id().to_owned();
    let remote_state = remote.runtime_state();
    let mut login = sessions::create(&remote_state, client_id.clone()).unwrap();
    login.client_id = client_id;
    login.r#type = plain_rs::features::sessions::SessionType::Login;
    remote_state.db.session_save(&login).unwrap();
    let mut paired = DPeer::new(
        &peer_id,
        "Fixture",
        "127.0.0.1",
        Some(port),
        DeviceType::Phone,
    );
    paired.status = PeerStatus::Paired;
    paired.public_key = plain_libs::utils::base64::base64_encode(&[7; 32]);
    paired.token = login.token;
    peers::save(&local.runtime_state().db, &[paired], SaveMode::Insert).unwrap();
    let sockets = local.native_sockets().unwrap();
    let scope = NativeRequestScope::default();
    let handle = sockets
        .open(SocketKind::Screen, Some(peer_id), scope.clone())
        .unwrap();
    let session = sockets.session(&handle).unwrap();
    assert_eq!(read(&session).await, vec![0]);
    session
        .send(br#"{"type":"screenMirrorControl","input":{"action":"BACK"}}"#.to_vec())
        .await
        .unwrap();
    let effect = tokio::time::timeout(Duration::from_secs(3), effects.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(effect["method"], "systemScreenMirrorControls");
    assert_eq!(effect["params"]["inputs"][0]["action"], "BACK");
    remote_state
        .main_ws
        .screen_events
        .send(WsEvent::broadcast_binary(
            "SCREEN_MIRROR_VIDEO",
            vec![0, 255, 1],
        ))
        .unwrap();
    assert_eq!(read(&session).await, vec![1, 0, 255, 1]);
    scope.close();
    removed(&sockets, &handle).await;
    let effect = tokio::time::timeout(Duration::from_secs(3), effects.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(effect["method"], "systemScreenMirrorResetTouch");
    local.stop_core().await;
    remote.stop_core().await;
    host.abort();
}
