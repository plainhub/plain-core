use super::*;
use crate::server::ContentServer;
use plain_rs::host::request_scope::NativeRequestScope;

const KEY: [u8; 32] = [7; 32];

fn fixture() -> (
    tempfile::TempDir,
    ContentServer,
    NativeEventStream,
    NativeRequestScope,
) {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open_with_cache(
        &directory.path().join("data"),
        &directory.path().join("cache"),
    )
    .unwrap();
    core.runtime_state()
        .prefs
        .set(
            plain_rs::prefs::pref_keys::URL_TOKEN,
            plain_rs::utils::base64::base64_encode(&KEY),
        )
        .unwrap();
    let scope = NativeRequestScope::default();
    let stream = core
        .native_graphql()
        .unwrap()
        .subscribe_events(scope.clone())
        .unwrap();
    (directory, core, stream, scope)
}

#[tokio::test]
async fn native_view_receives_binary_media_and_skips_text_host_and_other_client_frames() {
    let (_directory, core, mut stream, _scope) = fixture();
    let state = core.runtime_state();
    let events = &state.events;
    events
        .send(WsEvent::broadcast("CONTENT_CHANGED", "{}".into()))
        .unwrap();
    events
        .send(WsEvent::host_binary("SCREEN_MIRROR_VIDEO", vec![9]).unwrap())
        .unwrap();
    let mut other = WsEvent::broadcast_binary("SCREEN_MIRROR_VIDEO", vec![8]);
    other.target_cid = Some("other-client".into());
    events.send(other).unwrap();
    for kind in [
        "SCREEN_MIRROR_VIDEO",
        "SCREEN_MIRROR_AUDIO",
        "IMAGE_EDITOR_UPDATE",
    ] {
        events
            .send(WsEvent::broadcast_binary(kind, vec![1, 2, 3]))
            .unwrap();
        let frame = tokio::time::timeout(std::time::Duration::from_secs(3), stream.next_ui_frame())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            frame,
            plain_rs::protocols::websocket::encode_raw(kind, &[1, 2, 3])
        );
    }
    core.stop_core().await;
}

#[tokio::test]
async fn closing_a_view_wakes_its_pending_binary_read() {
    let (_directory, core, mut stream, scope) = fixture();
    let mut waiting = Box::pin(stream.next_ui_frame());
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(waiting.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    scope.close();
    assert!(waiting.await.unwrap().is_none());
    core.stop_core().await;
}

#[tokio::test]
async fn a_stopped_core_does_not_deliver_queued_frames() {
    let (_directory, core, mut stream, _scope) = fixture();
    core.runtime_state()
        .events
        .send(WsEvent::broadcast_binary("SCREEN_MIRROR_VIDEO", vec![1]))
        .unwrap();
    core.stop_core().await;
    assert!(stream.next_ui_frame().await.unwrap().is_none());
}

#[tokio::test]
async fn a_lagged_view_continues_with_retained_binary_frames() {
    let (_directory, core, mut stream, _scope) = fixture();
    let events = &core.runtime_state().events;
    for _ in 0..300 {
        events
            .send(WsEvent::broadcast("CONTENT_CHANGED", "{}".into()))
            .unwrap();
    }
    events
        .send(WsEvent::broadcast_binary("IMAGE_EDITOR_UPDATE", vec![4, 5]))
        .unwrap();
    let frame = tokio::time::timeout(std::time::Duration::from_secs(3), stream.next_ui_frame())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        frame,
        plain_rs::protocols::websocket::encode_raw("IMAGE_EDITOR_UPDATE", &[4, 5])
    );
    core.stop_core().await;
}

#[tokio::test]
async fn invalid_stored_event_keys_refuse_subscription() {
    let (_directory, core, _stream, _scope) = fixture();
    core.runtime_state()
        .prefs
        .set(
            plain_rs::prefs::pref_keys::URL_TOKEN,
            plain_rs::utils::base64::base64_encode(&KEY[..31]),
        )
        .unwrap();
    assert!(
        core.native_graphql()
            .unwrap()
            .subscribe_events(Default::default())
            .is_err()
    );
    core.stop_core().await;
}
