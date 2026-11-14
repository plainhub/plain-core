use super::*;
use crate::server::ContentServer;

#[tokio::test]
async fn search_waits_for_capacity_and_cancellation_releases_a_blocked_send() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let server = ContentServer::open(directory.path()).unwrap();
    let executor = server.native_graphql().unwrap();
    let handle = executor.open_subscriptions(Default::default()).unwrap();
    let SubscriptionSession::Local(session) = executor.subscription_session(handle).unwrap() else {
        panic!("local subscription required");
    };
    let scope = NativeRequestScope::default();
    for _ in 0..64 {
        session
            .queue_operation(
                ServerMessage::Pong { payload: None },
                &super::super::files::SPEC,
                scope.clone(),
            )
            .await
            .unwrap();
    }
    let send = session.queue_operation(
        ServerMessage::Pong { payload: None },
        &super::super::files::SPEC,
        scope.clone(),
    );
    tokio::pin!(send);
    assert!(futures_util::poll!(send.as_mut()).is_pending());
    assert!(!*session.closed.borrow());
    session.incoming.lock().await.messages.recv().await.unwrap();
    send.await.unwrap();
    let send = session.queue_operation(
        ServerMessage::Pong { payload: None },
        &super::super::files::SPEC,
        scope.clone(),
    );
    tokio::pin!(send);
    assert!(futures_util::poll!(send.as_mut()).is_pending());
    scope.close();
    assert_eq!(send.await.unwrap_err(), "Subscription operation closed");
    assert!(!*session.closed.borrow());
    executor.close_subscriptions(handle);
    server.shutdown().await;
}

#[cfg(feature = "http_transport")]
#[tokio::test]
async fn stopping_http_preserves_native_subscriptions_and_closes_remote_subscriptions() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let server = ContentServer::open(directory.path()).unwrap();
    let executor = server.native_graphql().unwrap();
    let handle = executor.open_subscriptions(Default::default()).unwrap();
    let state = server.runtime_state();
    let remote = state
        .subscription_sessions
        .open(
            state.clone(),
            RequestContext::remote(Some(state.clone())),
            "remote-client".into(),
            None,
            NativeRequestScope::default(),
        )
        .unwrap();
    server.stop_public().await;
    let native = executor
        .subscription_session(handle)
        .expect("native subscription must survive HTTP shutdown");
    native.send(r#"{"type":"ping"}"#).unwrap();
    assert_eq!(
        plain_libs::serde_json::from_str::<plain_libs::serde_json::Value>(&native.read().await.unwrap()).unwrap()["type"],
        "pong"
    );
    assert!(state.subscription_sessions.get(remote.id).is_err());
    server.stop_core().await;
    assert!(executor.subscription_session(handle).is_err());
    server.shutdown().await;
}

#[tokio::test]
async fn scan_events_wait_for_capacity_without_closing_the_connection_or_blocking_heartbeat() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let server = ContentServer::open(directory.path()).unwrap();
    let executor = server.native_graphql().unwrap();
    let handle = executor.open_subscriptions(Default::default()).unwrap();
    let SubscriptionSession::Local(session) = executor.subscription_session(handle).unwrap() else {
        panic!("local subscription required");
    };
    let spec = plain_rs::infrastructure::protocols::graphql::events::EVENTS
        .iter()
        .find(|spec| spec.field == "fileIndexScanProgressUpdated")
        .unwrap();
    let scope = NativeRequestScope::default();
    for _ in 0..64 {
        session
            .queue_operation(
                ServerMessage::Complete {
                    id: "old-page".into(),
                },
                spec,
                scope.clone(),
            )
            .await
            .unwrap();
    }
    let send = session.queue_operation(
        ServerMessage::Complete {
            id: "old-page".into(),
        },
        spec,
        scope.clone(),
    );
    tokio::pin!(send);
    assert!(futures_util::poll!(send.as_mut()).is_pending());
    assert!(!*session.closed.borrow());
    session
        .send(r#"{"type":"ping","payload":{"probe":1}}"#)
        .unwrap();
    let pong: plain_libs::serde_json::Value = plain_libs::serde_json::from_str(&session.read().await.unwrap()).unwrap();
    assert_eq!(pong["type"], "pong");
    assert_eq!(pong["payload"]["probe"], 1);
    assert!(futures_util::poll!(send.as_mut()).is_pending());
    scope.close();
    assert_eq!(send.await.unwrap_err(), "Subscription operation closed");
    session.send(r#"{"type":"ping"}"#).unwrap();
    let pong: plain_libs::serde_json::Value = plain_libs::serde_json::from_str(&session.read().await.unwrap()).unwrap();
    assert_eq!(pong["type"], "pong");
    assert!(!*session.closed.borrow());
    executor.close_subscriptions(handle);
    server.shutdown().await;
}

#[tokio::test]
async fn closing_a_session_releases_a_blocked_event_producer() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let server = ContentServer::open(directory.path()).unwrap();
    let executor = server.native_graphql().unwrap();
    let handle = executor.open_subscriptions(Default::default()).unwrap();
    let SubscriptionSession::Local(session) = executor.subscription_session(handle).unwrap() else {
        panic!("local subscription required")
    };
    let spec = plain_rs::infrastructure::protocols::graphql::events::EVENTS
        .iter()
        .find(|spec| spec.field == "contentChanged")
        .unwrap();
    let scope = NativeRequestScope::default();
    for _ in 0..64 {
        session
            .queue_operation(
                ServerMessage::Complete {
                    id: "old-page".into(),
                },
                spec,
                scope.clone(),
            )
            .await
            .unwrap();
    }
    let send = session.queue_operation(
        ServerMessage::Complete {
            id: "old-page".into(),
        },
        spec,
        scope,
    );
    tokio::pin!(send);
    assert!(futures_util::poll!(send.as_mut()).is_pending());
    executor.close_subscriptions(handle);
    assert_eq!(send.await.unwrap_err(), "Subscription session closed");
    server.shutdown().await;
}

#[tokio::test]
async fn peer_subscriptions_share_the_session_bound_and_closed_owners_are_rejected() {
    use plain_rs::{
        features::chat::enums::{DeviceType, PeerStatus},
        infrastructure::{
            db::{
                DPeer,
                chat_store::{SaveMode, peers},
            },
            protocols::graphql::PeerSubscriptionTarget,
        },
    };
    let directory = plain_libs::temporary::tempdir().unwrap();
    let server = ContentServer::open(directory.path()).unwrap();
    let state = server.runtime_state();
    let executor = server.native_graphql().unwrap();
    let scope = NativeRequestScope::default();
    scope.close();
    assert!(executor.open_subscriptions(scope).is_err());
    let mut peer = DPeer::new("peer", "Fixture", "127.0.0.1", None, DeviceType::Phone);
    peer.status = PeerStatus::Paired;
    peer.public_key = "fixture-identity".into();
    peer.token = plain_libs::utils::base64::base64_encode(&[1; 32]);
    peers::save(&state.db, &[peer.clone()], SaveMode::Insert).unwrap();
    for _ in 0..128 {
        executor.open_subscriptions(Default::default()).unwrap();
    }
    assert_eq!(
        state
            .subscription_sessions
            .open_peer_with_pin(
                state.clone(),
                PeerSubscriptionTarget {
                    peer_id: peer.id,
                    public_key: peer.public_key
                },
                Default::default(),
                Some([1; 32])
            )
            .unwrap_err(),
        "Subscription session capacity reached"
    );
    assert_eq!(
        state.subscription_sessions.entries.lock().unwrap().len(),
        128
    );
    server.stop_core().await;
}

#[tokio::test]
async fn request_and_response_limits_fill_exactly_one_mib_encrypted_frames() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let server = ContentServer::open(directory.path()).unwrap();
    let executor = server.native_graphql().unwrap();
    let handle = executor.open_subscriptions(Default::default()).unwrap();
    let SubscriptionSession::Local(session) = executor.subscription_session(handle).unwrap() else {
        panic!("local subscription required");
    };
    let empty = r#"{"type":"ping","payload":""}"#;
    let payload = "x".repeat(1024 * 1024 - 48 - empty.len());
    let encoded = empty.replacen("\"\"", &format!("\"{payload}\""), 1);
    session.send(&encoded).unwrap();
    let response = session.read().await.unwrap();
    assert_eq!(response.len(), encoded.len());
    use plain_rs::infrastructure::protocols::websocket::{Role, Stream};
    let mut stream = Stream::new("/graphql", "client", b"init", b"ack", Role::Server).unwrap();
    assert_eq!(
        stream.encode(response.as_bytes(), &[7; 32]).unwrap().len(),
        1024 * 1024
    );
    assert_eq!(
        session.send(&(encoded + " ")).unwrap_err(),
        "Subscription request too large"
    );
    assert_eq!(
        session
            .encode(
                ServerMessage::Pong {
                    payload: Some(plain_libs::serde_json::Value::String(payload + "x")),
                },
                None,
                None
            )
            .err()
            .unwrap(),
        "Subscription response too large"
    );
    executor.close_subscriptions(handle);
    server.shutdown().await;
}
