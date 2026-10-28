use super::*;
use crate::assembly::server::ContentServer;
use std::sync::Arc;

#[tokio::test]
async fn closed_native_page_cancels_its_pending_main_schema_request_before_radio_acquisition() {
    let directory = tempfile::tempdir().unwrap();
    let prefs =
        Arc::new(plain_rs::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap());
    let server = ContentServer::start(
        directory.path(),
        &plain_rs::utils::base64::base64_encode(&[7; 32]),
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
                        uuid::Uuid::new_v4()
                    )),
                    scope,
                )
                .await
        }
    });
    let request = requests.recv().await.unwrap();
    assert_eq!(request["method"], "systemCapabilities");
    scope.close();
    let response = pending.await.unwrap();
    assert!(!response.errors.is_empty());
    assert!(scope.is_closed());
    assert_eq!(state.mdns.snapshot()["receiver"], false);
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
    let directory = tempfile::tempdir().unwrap();
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap());
    let server = ContentServer::start_core(
        directory.path(),
        &plain_rs::utils::base64::base64_encode(&[7; 32]),
        prefs,
    ).unwrap();
    let executor = server.native_graphql().unwrap();
    let scope = NativeRequestScope::default();
    let id = executor.open_subscriptions(scope.clone()).unwrap();
    let session = executor.subscription_session(id).unwrap();
    session.send(r#"{"type":"subscribe","id":"content","payload":{"query":"subscription { contentChanged }"}}"#).unwrap();
    let ready: serde_json::Value = serde_json::from_str(&tokio::time::timeout(std::time::Duration::from_secs(5), session.read()).await.unwrap().unwrap()).unwrap();
    assert_eq!(ready["payload"]["extensions"]["subscriptionReady"], true);
    server.runtime_state().events.send(plain_rs::events::WsEvent::broadcast("CONTENT_CHANGED", "{}".into())).unwrap();
    let next: serde_json::Value = serde_json::from_str(&tokio::time::timeout(std::time::Duration::from_secs(5), session.read()).await.unwrap().unwrap()).unwrap();
    assert_eq!(next["id"], "content");
    assert_eq!(next["payload"]["data"]["contentChanged"], true);
    scope.close();
    assert!(tokio::time::timeout(std::time::Duration::from_secs(5), session.read()).await.unwrap().is_err());
    executor.close_subscriptions(id);
    server.stop_core().await;
}

#[tokio::test]
async fn chat_subscriptions_deliver_messages_without_status_details_and_remain_active() {
    let directory = tempfile::tempdir().unwrap();
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap());
    let server = ContentServer::start_core(
        directory.path(),
        &plain_rs::utils::base64::base64_encode(&[7; 32]),
        prefs,
    ).unwrap();
    let executor = server.native_graphql().unwrap();
    let id = executor.open_subscriptions(NativeRequestScope::default()).unwrap();
    let session = executor.subscription_session(id).unwrap();
    for (field, event) in [("messagesCreated", "MESSAGE_CREATED"), ("messagesUpdated", "MESSAGE_UPDATED")] {
        session.send(&serde_json::json!({"type": "subscribe", "id": field, "payload": {
            "query": format!("subscription {{ {field} {{ id status statusData channelId }} }}")
        }}).to_string()).unwrap();
        let ready: serde_json::Value = serde_json::from_str(&tokio::time::timeout(std::time::Duration::from_secs(5), session.read()).await.unwrap().unwrap()).unwrap();
        assert_eq!(ready["payload"]["extensions"]["subscriptionReady"], true);
        for details in [None, Some(r#"{"results":[]}"#.to_string()), None] {
            let row = plain_rs::db::DChat {
                id: "message".into(), from_id: "me".into(), to_id: "peer".into(),
                channel_id: None, content: r#"{"type":"TEXT","value":{"text":"hello"}}"#.into(),
                created_at: "2026-01-01T00:00:00Z".into(), updated_at: "2026-01-01T00:00:00Z".into(),
                status: plain_rs::chat::enums::ChatStatus::Sent, status_data: details.clone(),
            };
            server.runtime_state().events.send(plain_rs::events::WsEvent::broadcast(
                event, serde_json::json!([plain_rs::chat::service::chat_to_json(&row)]).to_string(),
            )).unwrap();
            let next: serde_json::Value = serde_json::from_str(&tokio::time::timeout(std::time::Duration::from_secs(5), session.read()).await.unwrap().unwrap()).unwrap();
            assert!(next["payload"]["errors"].is_null(), "{next}");
            assert_eq!(next["payload"]["data"][field][0]["id"], "message", "{next}");
            assert_eq!(next["payload"]["data"][field][0]["statusData"], details.unwrap_or_default());
            assert!(next["payload"]["data"][field][0]["channelId"].is_null());
        }
    }
    executor.close_subscriptions(id);
    server.stop_core().await;
}
