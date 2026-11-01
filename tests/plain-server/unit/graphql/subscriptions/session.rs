use super::*;
use crate::server::ContentServer;

#[tokio::test]
async fn search_waits_for_capacity_and_cancellation_releases_a_blocked_send() {
    let directory = tempfile::tempdir().unwrap();
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
    session.incoming.lock().await.recv().await.unwrap();
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
    let directory = tempfile::tempdir().unwrap();
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
