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
        serde_json::from_str::<serde_json::Value>(&native.read().await.unwrap()).unwrap()["type"],
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
        .find(|spec| spec.field == "mediaScanProgress")
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
    let pong: serde_json::Value = serde_json::from_str(&session.read().await.unwrap()).unwrap();
    assert_eq!(pong["type"], "pong");
    assert_eq!(pong["payload"]["probe"], 1);
    assert!(futures_util::poll!(send.as_mut()).is_pending());
    scope.close();
    assert_eq!(send.await.unwrap_err(), "Subscription operation closed");
    session.send(r#"{"type":"ping"}"#).unwrap();
    let pong: serde_json::Value = serde_json::from_str(&session.read().await.unwrap()).unwrap();
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
