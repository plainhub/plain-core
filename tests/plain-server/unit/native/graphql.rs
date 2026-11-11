use super::*;
use crate::server::ContentServer;
use std::sync::Arc;

#[tokio::test]
async fn closed_native_page_cancels_its_pending_main_schema_request_before_radio_acquisition() {
    let _runtime = plain_rs::features::services::discovery::mdns_runtime::TEST_LOCK
        .lock()
        .await;
    let directory = plain_libs::temporary::tempdir().unwrap();
    let prefs = Arc::new(
        plain_rs::features::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap(),
    );
    prefs.set("client_id", "fixture-native-scope").unwrap();
    prefs
        .set("mdns_hostname", "fixture-native-scope.local")
        .unwrap();
    let server = ContentServer::start(
        directory.path(),
        &plain_libs::utils::base64::base64_encode(&[7; 32]),
        prefs,
    )
    .unwrap();
    let state = server.runtime_state();
    let (generation, mut requests) = state.host.connect();
    let executor = server.native_graphql().unwrap();
    let scope = NativeRequestScope::default();
    let pending = tokio::spawn({
        let scope = scope.clone();
        async move {
            executor
                .execute_scoped(
                    Request::new(format!(
                        r#"mutation{{acquireDiscovery(token:"{}")}}"#,
                        plain_libs::utils::uuid::Uuid::new_v4()
                    )),
                    scope,
                )
                .await
        }
    });
    let request = tokio::time::timeout(std::time::Duration::from_secs(5), requests.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(request["method"], "mdnsMulticast");
    assert_eq!(request["params"]["acquire"], true);
    assert!(!request["params"]["lease"].as_str().unwrap().is_empty());
    scope.close();
    let response = tokio::time::timeout(std::time::Duration::from_secs(5), pending)
        .await
        .unwrap()
        .unwrap();
    assert!(!response.errors.is_empty());
    assert!(scope.is_closed());
    assert_eq!(state.mdns.snapshot()["receiver"], false);
    let release = requests.try_recv().unwrap();
    assert_eq!(release["method"], "mdnsMulticast");
    assert_eq!(release["params"]["acquire"], false);
    assert_eq!(release["params"]["lease"], request["params"]["lease"]);
    assert!(requests.try_recv().is_err());
    state.host.disconnect(generation);
    server.shutdown().await;
}

#[tokio::test]
async fn closing_an_old_native_scope_does_not_close_a_new_page_scope() {
    let old = NativeRequestScope::default();
    let new = NativeRequestScope::default();
    let old_request = old.clone();
    old.close();
    assert!(old_request.is_closed());
    assert!(!new.is_closed());
    assert_ne!(old.id(), new.id());
}

#[tokio::test]
async fn native_subscription_delivers_without_an_http_listener() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let prefs = Arc::new(
        plain_rs::features::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap(),
    );
    let server = ContentServer::start_core(
        directory.path(),
        &plain_libs::utils::base64::base64_encode(&[7; 32]),
        prefs,
    )
    .unwrap();
    let executor = server.native_graphql().unwrap();
    let scope = NativeRequestScope::default();
    let id = executor.open_subscriptions(scope.clone()).unwrap();
    let session = executor.subscription_session(id).unwrap();
    session.send(r#"{"type":"subscribe","id":"content","payload":{"query":"subscription { contentChanged }"}}"#).unwrap();
    let ready: serde_json::Value = serde_json::from_str(
        &tokio::time::timeout(std::time::Duration::from_secs(5), session.read())
            .await
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(ready["payload"]["extensions"]["subscriptionReady"], true);
    server
        .runtime_state()
        .events
        .send(plain_rs::infrastructure::events::WsEvent::broadcast(
            "CONTENT_CHANGED",
            "{}".into(),
        ))
        .unwrap();
    let next: serde_json::Value = serde_json::from_str(
        &tokio::time::timeout(std::time::Duration::from_secs(5), session.read())
            .await
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(next["id"], "content");
    assert_eq!(next["payload"]["data"]["contentChanged"], true);
    scope.close();
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(5), session.read())
            .await
            .unwrap()
            .is_err()
    );
    executor.close_subscriptions(id);
    server.stop_core().await;
}

#[tokio::test]
async fn chat_subscriptions_deliver_messages_without_status_details_and_remain_active() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let prefs = Arc::new(
        plain_rs::features::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap(),
    );
    let server = ContentServer::start_core(
        directory.path(),
        &plain_libs::utils::base64::base64_encode(&[7; 32]),
        prefs,
    )
    .unwrap();
    let executor = server.native_graphql().unwrap();
    let id = executor
        .open_subscriptions(NativeRequestScope::default())
        .unwrap();
    let session = executor.subscription_session(id).unwrap();
    for (field, event) in [
        ("messagesCreated", "MESSAGE_CREATED"),
        ("messagesUpdated", "MESSAGE_UPDATED"),
    ] {
        session.send(&serde_json::json!({"type": "subscribe", "id": field, "payload": {
            "query": format!("subscription {{ {field} {{ id status statusData channelId }} }}")
        }}).to_string()).unwrap();
        let ready: serde_json::Value = serde_json::from_str(
            &tokio::time::timeout(std::time::Duration::from_secs(5), session.read())
                .await
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(ready["payload"]["extensions"]["subscriptionReady"], true);
        for details in [None, Some(r#"{"results":[]}"#.to_string()), None] {
            let row = plain_rs::infrastructure::db::DChat {
                id: "message".into(),
                from_id: "me".into(),
                to_id: "peer".into(),
                channel_id: None,
                content: r#"{"type":"TEXT","value":{"text":"hello"}}"#.into(),
                created_at: "2026-01-01T00:00:00Z".into(),
                updated_at: "2026-01-01T00:00:00Z".into(),
                status: plain_rs::features::chat::enums::ChatStatus::Sent,
                status_data: details.clone(),
            };
            server
                .runtime_state()
                .events
                .send(plain_rs::infrastructure::events::WsEvent::broadcast(
                    event,
                    serde_json::json!([plain_rs::features::chat::service::chat_to_json(&row)])
                        .to_string(),
                ))
                .unwrap();
            let next: serde_json::Value = serde_json::from_str(
                &tokio::time::timeout(std::time::Duration::from_secs(5), session.read())
                    .await
                    .unwrap()
                    .unwrap(),
            )
            .unwrap();
            assert!(next["payload"]["errors"].is_null(), "{next}");
            assert_eq!(next["payload"]["data"][field][0]["id"], "message", "{next}");
            assert_eq!(
                next["payload"]["data"][field][0]["statusData"],
                details.unwrap_or_default()
            );
            assert!(next["payload"]["data"][field][0]["channelId"].is_null());
        }
    }
    executor.close_subscriptions(id);
    server.stop_core().await;
}

#[tokio::test]
async fn pending_multicast_requests_remain_independent_until_permission_is_granted() {
    let _runtime = plain_rs::features::services::discovery::mdns_runtime::TEST_LOCK
        .lock()
        .await;
    let directory = plain_libs::temporary::tempdir().unwrap();
    let first = ContentServer::open(&directory.path().join("first")).unwrap();
    let second = ContentServer::open(&directory.path().join("second")).unwrap();
    let first_state = first.runtime_state();
    let second_state = second.runtime_state();
    first_state
        .prefs
        .set("mdns_hostname", "fixture-first-runtime.local")
        .unwrap();
    second_state
        .prefs
        .set("mdns_hostname", "fixture-second-runtime.local")
        .unwrap();
    let (first_generation, mut first_requests) = first_state.host.connect();
    let (second_generation, mut second_requests) = second_state.host.connect();
    let acquire = || {
        Request::new(format!(
            r#"mutation{{acquireDiscovery(token:"{}")}}"#,
            plain_libs::utils::uuid::Uuid::new_v4()
        ))
    };
    let first_scope = NativeRequestScope::default();
    let first_executor = first.native_graphql().unwrap();
    let first_pending = tokio::spawn({
        let scope = first_scope.clone();
        let request = acquire();
        async move { first_executor.execute_scoped(request, scope).await }
    });
    let first_request =
        tokio::time::timeout(std::time::Duration::from_secs(5), first_requests.recv())
            .await
            .unwrap()
            .unwrap();
    assert_eq!(first_request["method"], "mdnsMulticast");
    assert_eq!(first_request["params"]["acquire"], true);
    let second_scope = NativeRequestScope::default();
    let second_executor = second.native_graphql().unwrap();
    let second_pending = tokio::spawn({
        let scope = second_scope.clone();
        let request = acquire();
        async move { second_executor.execute_scoped(request, scope).await }
    });
    let second_request =
        tokio::time::timeout(std::time::Duration::from_secs(5), second_requests.recv())
            .await
            .unwrap()
            .unwrap();
    assert_eq!(second_request["method"], "mdnsMulticast");
    assert_eq!(second_request["params"]["acquire"], true);
    assert_ne!(
        first_request["params"]["lease"],
        second_request["params"]["lease"]
    );
    first_scope.close();
    assert!(
        !tokio::time::timeout(std::time::Duration::from_secs(5), first_pending)
            .await
            .unwrap()
            .unwrap()
            .errors
            .is_empty()
    );
    assert!(matches!(
        second_requests.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    ));
    second_scope.close();
    assert!(
        !tokio::time::timeout(std::time::Duration::from_secs(5), second_pending)
            .await
            .unwrap()
            .unwrap()
            .errors
            .is_empty()
    );
    assert_eq!(first_state.mdns.snapshot()["receiver"], false);
    assert_eq!(second_state.mdns.snapshot()["receiver"], false);
    for (requests, acquire) in [
        (&mut first_requests, first_request),
        (&mut second_requests, second_request),
    ] {
        let release = requests.try_recv().unwrap();
        assert_eq!(release["method"], "mdnsMulticast");
        assert_eq!(release["params"]["acquire"], false);
        assert_eq!(release["params"]["lease"], acquire["params"]["lease"]);
        assert!(requests.try_recv().is_err());
    }
    first_state.host.disconnect(first_generation);
    second_state.host.disconnect(second_generation);
    first.stop_core().await;
    second.stop_core().await;
}

#[test]
fn remote_transport_errors_preserve_session_and_permission_status() {
    use plain_rs::features::services::peers::management_http::AuthorizationError;
    assert_eq!(
        peer_error(AuthorizationError::Session.into()),
        "unauthorized"
    );
    assert_eq!(
        peer_error(AuthorizationError::Permission.into()),
        "remote_access_disabled"
    );
    assert_eq!(
        peer_error(plain_libs::anyhow::anyhow!("fixture failure")),
        "fixture failure"
    );
}
