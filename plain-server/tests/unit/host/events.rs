use super::*;
use crate::assembly::server::ContentServer;
use std::{sync::Arc, time::Duration};

fn server(dir: &tempfile::TempDir) -> ContentServer {
    ContentServer::start(
        dir.path(),
        &plain_rs::utils::base64::base64_encode(&[9; 32]),
        Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap()),
    )
    .unwrap()
}

#[tokio::test]
async fn native_events_enforce_direction_and_reconcile_lag() {
    let dir = tempfile::tempdir().unwrap();
    let server = server(&dir);
    let bridge = server.host_events().unwrap();
    let mut public = server.runtime_state().events.subscribe();
    assert!(public.try_recv().is_err());
    for kind in [
        "MESSAGE_CREATED",
        "SMS_SEND_RESULT",
        "PAIRING_SUCCESS",
        "CONTENT_CHANGED",
        "CLIPBOARD_CHANGED",
        "UNKNOWN_EVENT",
        "IMAGE_INDEX_ERROR_REPORTED",
        "NETWORK_CHANGED",
        "AUDIO_LIBRARY_UPDATED",
        plain_rs::events::MEDIA_LIBRARY_UPDATED,
        "HTTP_SERVER_STATE_CHANGED",
        "MEDIA_ACTION_FINISHED",
        "MEDIA_CATALOG_CHANGED",
    ] {
        assert!(bridge.publish_text(kind, "{}".into()).await.is_err());
        assert!(bridge.publish_binary(kind, vec![]).is_err());
    }
    let mut screen = server.runtime_state().main_ws.screen_events.subscribe();
    for kind in HOST_TEXT_TYPES {
        if matches!(
            *kind,
            "PAIRING_REQUEST_REPORTED"
                | "PAIRING_RESPONSE_REPORTED"
                | "PAIRING_CANCEL_REPORTED"
                | "NOTIFICATION_REPORTED"
                | "CLIPBOARD_REPORTED"
                | "AUDIO_PROGRESS_REPORTED"
                | "AUDIO_STARTED_REPORTED"
                | "AUDIO_COMPLETED_REPORTED"
                | "DLNA_POSITION_REPORTED"
                | "DLNA_PLAYBACK_REPORTED"
                | "DLNA_SEEK_CONSUMED_REPORTED"
                | "DLNA_STOPPED_REPORTED"
                | "IMAGE_CATALOG_CHANGED"
                | "NETWORK_INTERFACES_CHANGED"
                | "BLE_ADVERTISEMENT_REPORTED"
                | "BLE_SCAN_STATE_REPORTED"
                | "BLE_ADVERTISING_FACTS_REPORTED"
                | "SMS_PART_SENT_REPORTED"
        ) {
            assert!(
                bridge.publish_text(kind, "{}".into()).await.is_err(),
                "{kind}"
            );
            assert!(public.try_recv().is_err(), "{kind}");
        } else {
            bridge.publish_text(kind, "{}".into()).await.unwrap();
            let receiver = if crate::http::websocket::runtime::is_screen_event(kind) {
                &mut screen
            } else {
                &mut public
            };
            assert!(receiver.recv().await.unwrap().is_host_event());
        }
    }
    for kind in HOST_BINARY_TYPES {
        bridge.publish_binary(kind, vec![1, 2, 3]).unwrap();
        assert_eq!(
            screen.recv().await.unwrap().binary_payload,
            Some(vec![1, 2, 3])
        );
    }
    for _ in 0..300 {
        bridge
            .publish_binary("SCREEN_MIRROR_VIDEO", vec![1, 2, 3])
            .unwrap();
    }
    assert!(matches!(
        screen.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_))
    ));
    assert!(matches!(
        public.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
    server.shutdown().await;
}
#[tokio::test]
async fn native_requests_reply_and_replacement_cancel_pending_calls() {
    let dir = tempfile::tempdir().unwrap();
    let server = server(&dir);
    let bridge = server.host_events().unwrap();
    let host = server.runtime_state().host.clone();
    let call_host = host.clone();
    let task = tokio::spawn(async move { call_host.call("fixture", json!({})).await });
    let request = loop {
        let request = bridge.next_request().await.unwrap();
        if request["method"] == "fixture" {
            break request;
        }
        let result = if request["method"] == "systemCapabilities" { json!([]) } else { json!(null) };
        bridge
            .reply(json!({"id":request["id"],"result":result}))
            .unwrap();
    };
    bridge
        .reply(json!({"id":request["id"],"result":42}))
        .unwrap();
    assert_eq!(task.await.unwrap().unwrap(), json!(42));
    let task = tokio::spawn(async move { host.call("fixture", json!({})).await });
    loop {
        let request = bridge.next_request().await.unwrap();
        if request["method"] == "fixture" {
            break;
        }
        let result = if request["method"] == "systemCapabilities" { json!([]) } else { json!(null) };
        bridge
            .reply(json!({"id":request["id"],"result":result}))
            .unwrap();
    }
    let replacement = server.host_events().unwrap();
    assert!(task.await.unwrap().unwrap_err().contains("replaced"));
    assert!(
        bridge
            .publish_text(HOST_TEXT_TYPES[0], "{}".into())
            .await
            .is_err()
    );
    drop(bridge);
    assert!(
        server
            .runtime_state()
            .host
            .is_current(replacement.generation)
    );
    server.shutdown().await;
}

#[tokio::test]
async fn replacing_a_host_wakes_a_disconnection_waiter() {
    let dir = tempfile::tempdir().unwrap();
    let server = server(&dir);
    let original = server.host_events().unwrap();
    let mut waiting = Box::pin(original.wait_disconnected());
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(waiting.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    }).await;
    let replacement = server.host_events().unwrap();
    tokio::time::timeout(Duration::from_secs(3), waiting).await.unwrap();
    assert!(server.runtime_state().host.is_current(replacement.generation));
    server.shutdown().await;
}

#[tokio::test]
async fn replaced_hosts_discard_requests_and_events_already_in_the_queue() {
    let dir = tempfile::tempdir().unwrap();
    let server = server(&dir);
    let original = server.host_events().unwrap();
    server
        .runtime_state()
        .events
        .send(WsEvent::broadcast("CONTENT_CHANGED", "{}".into()))
        .unwrap();
    let replacement = server.host_events().unwrap();
    assert!(original.next_request().await.is_none());
    original.wait_disconnected().await;
    assert!(
        server
            .runtime_state()
            .host
            .is_current(replacement.generation)
    );
    server.shutdown().await;
}

#[tokio::test]
async fn raw_ble_advertisements_are_consumed_without_public_rebroadcast() {
    let dir = tempfile::tempdir().unwrap();
    let server = server(&dir);
    let bridge = server.host_events().unwrap();
    let state = server.runtime_state();
    let mut public = state.events.subscribe();
    let scan = uuid::Uuid::new_v4().to_string();
    bridge
        .publish_text(
            "BLE_SCAN_STATE_REPORTED",
            serde_json::json!({"scanId":scan,"active":true}).to_string(),
        )
        .await
        .unwrap();
    bridge
        .publish_text(
            "BLE_ADVERTISEMENT_REPORTED",
            serde_json::json!({"scanId":scan,"shortId":"invalid"}).to_string(),
        )
        .await
        .unwrap();
    assert!(public.try_recv().is_err());
    bridge
        .publish_text(
            "BLE_SCAN_STATE_REPORTED",
            serde_json::json!({"scanId":scan,"active":false}).to_string(),
        )
        .await
        .unwrap();
    assert!(
        bridge
            .publish_text(
                "BLE_ADVERTISEMENT_REPORTED",
                serde_json::json!({"scanId":scan,"shortId":"0001020304050607"}).to_string()
            )
            .await
            .is_err()
    );
    bridge
        .publish_text("IMAGE_CATALOG_CHANGED", "null".into())
        .await
        .unwrap();
    assert!(public.try_recv().is_err());
    let mut network = server.runtime_state().network_revision.subscribe();
    bridge
        .publish_text("NETWORK_INTERFACES_CHANGED", "null".into())
        .await
        .unwrap();
    assert!(network.has_changed().unwrap());
    assert_eq!(*network.borrow_and_update(), 1);
    assert!(public.try_recv().is_err());
    assert!(
        bridge
            .publish_text("NETWORK_INTERFACES_CHANGED", "{}".into())
            .await
            .is_err()
    );
    server.shutdown().await;
}

#[tokio::test]
async fn client_request_errors_cannot_overwrite_shared_image_model_state() {
    let directory = tempfile::tempdir().unwrap();
    let server = server(&directory);
    let bridge = server.host_events().unwrap();
    let state = server.runtime_state();
    let before = state.image_models.snapshot();
    assert!(
        bridge
            .publish_text(
                "IMAGE_INDEX_ERROR_REPORTED",
                json!("Client transport failed").to_string()
            )
            .await
            .is_err()
    );
    let after = state.image_models.snapshot();
    assert_eq!(before.status.error_message, after.status.error_message);
    assert_eq!(before.status.status, after.status.status);
    assert_eq!(before.version, after.version);
    server.shutdown().await;
}

#[tokio::test]
async fn scan_facts_are_validated_and_scoped_to_the_connected_host() {
    let dir = tempfile::tempdir().unwrap();
    let server = server(&dir);
    let old = server.host_events().unwrap();
    let id = uuid::Uuid::from_u128(1).to_string();
    let active = serde_json::json!({"scanId":id,"active":true}).to_string();
    old.publish_text("BLE_SCAN_STATE_REPORTED", active.clone())
        .await
        .unwrap();
    let bridge = server.host_events().unwrap();
    assert!(
        old.publish_text("BLE_SCAN_STATE_REPORTED", active.clone())
            .await
            .is_err()
    );
    bridge
        .publish_text("BLE_SCAN_STATE_REPORTED", active)
        .await
        .unwrap();
    drop(old);
    let state = server.runtime_state();
    assert!(matches!(
        state.ble_scans.seen(&id, "0001020304050607").unwrap(),
        plain_rs::discovery::nearby_scan::Step::Read { .. }
    ));
    for invalid in [
        serde_json::json!({"scanId":"invalid","active":true}),
        serde_json::json!({"scanId":id,"active":"true"}),
        serde_json::json!({"scanId":id,"active":true,"extra":true}),
    ] {
        assert!(
            bridge
                .publish_text("BLE_SCAN_STATE_REPORTED", invalid.to_string())
                .await
                .is_err()
        );
    }
    drop(ServingSession(&bridge));
    assert!(state.ble_scans.seen(&id, "0001020304050607").is_err());
    server.shutdown().await;
}

#[tokio::test]
async fn preference_updates_send_only_changed_values_and_skip_unchanged_values() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let bridge = Arc::new(core.host_events().unwrap());
    state.prefs.set("http_port", 8181).unwrap();
    state.prefs.set("feed_auto_refresh_interval", 900).unwrap();
    state.prefs.set("pkg_sort_by", "NAME_DESC").unwrap();
    let mut update = Box::pin(bridge.prefs_changed());
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(update.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    loop {
        let request = bridge.next_request().await.unwrap();
        let is_preferences = request["method"] == "prefsChanged";
        if is_preferences {
            assert_eq!(
                request["params"],
                json!({"values":{"http_port":8181,"feed_auto_refresh_interval":900,"pkg_sort_by":"NAME_DESC"}})
            );
        }
        bridge
            .reply(json!({"id":request["id"],"result":null}))
            .unwrap();
        if is_preferences {
            break;
        }
    }
    update.await.unwrap();
    let mut unchanged = Box::pin(bridge.prefs_changed());
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(unchanged.as_mut(), context).is_ready());
        std::task::Poll::Ready(())
    })
    .await;
    drop(unchanged);
    core.stop_core().await;
}

#[tokio::test]
async fn failed_preference_delivery_returns_once_and_does_not_replay_the_change() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let bridge = Arc::new(core.host_events().unwrap());
    state.prefs.set("http_port", 8181).unwrap();
    let mut update = Box::pin(bridge.prefs_changed());
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(update.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    loop {
        let request = bridge.next_request().await.unwrap();
        let is_preferences = request["method"] == "prefsChanged";
        let response = if is_preferences {
            json!({"id":request["id"],"error":"platform failure"})
        } else {
            json!({"id":request["id"],"result":null})
        };
        bridge.reply(response).unwrap();
        if is_preferences {
            break;
        }
    }
    std::future::poll_fn(
        |context| match std::future::Future::poll(update.as_mut(), context) {
            std::task::Poll::Ready(result) => {
                assert_eq!(result.unwrap_err(), "platform failure");
                std::task::Poll::Ready(())
            }
            std::task::Poll::Pending => panic!("failed preference delivery did not finish"),
        },
    )
    .await;
    drop(update);
    bridge.prefs_changed().await.unwrap();
    core.stop_core().await;
}

#[tokio::test]
async fn preference_read_failure_does_not_replace_the_running_host() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let bridge = core.host_events().unwrap();
    state.prefs.set("http_port", "invalid").unwrap();
    assert!(core.host_events().is_err());
    assert_eq!(state.host.generation(), Some(bridge.generation));
    assert!(bridge.prefs_changed().await.is_err());
    core.stop_core().await;
    assert!(core.host_events().is_err());
}
