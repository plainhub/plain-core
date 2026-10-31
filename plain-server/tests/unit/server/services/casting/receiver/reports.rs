use crate::server::ContentServer;
use plain_rs::services::casting::receiver::types::DlnaMediaType;
use plain_rs::services::casting::receiver::types::DlnaPlaybackState;
use plain_rs::services::casting::receiver::types::PendingCastRequest;
use serde_json::json;
use std::sync::Arc;

fn fixture() -> (tempfile::TempDir, ContentServer) {
    let directory = tempfile::tempdir().unwrap();
    let prefs =
        Arc::new(plain_rs::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap());
    let core = ContentServer::start(
        directory.path(),
        &plain_rs::utils::base64::base64_encode(&[7; 32]),
        prefs,
    )
    .unwrap();
    (directory, core)
}

#[tokio::test]
async fn position_facts_keep_exact_milliseconds_without_snapshots_or_public_refreshes() {
    let (_directory, core) = fixture();
    let state = core.runtime_state().core;
    let bridge = core.host_events().unwrap();
    state.dlna.state.write().await.media_uri = "https://example.test/current".into();
    let version = state.dlna.state.read().await.version;
    let mut public = state.events.subscribe();
    let mut changes = state.dlna.changes();
    for (position_ms, duration_ms) in [
        (5_000_000_001_i64, i64::MAX),
        (i64::MAX, i64::MAX),
        (-1, -2),
    ] {
        bridge.publish_text("DLNA_POSITION_REPORTED", json!({
            "mediaUri":"https://example.test/current", "positionMs":position_ms, "durationMs":duration_ms,
        }).to_string()).await.unwrap();
        let renderer = state.dlna.state.read().await;
        assert_eq!(renderer.current_position_ms, position_ms.max(0));
        assert_eq!(renderer.duration_ms, duration_ms.max(0));
        assert_eq!(renderer.version, version);
        assert!(changes.try_recv().is_err());
        assert!(public.try_recv().is_err());
    }
    for payload in [
        json!({}),
        json!({"mediaUri":"", "positionMs":1, "durationMs":2}),
        json!({"mediaUri":"https://example.test/current", "positionMs":1.5, "durationMs":2}),
        json!({"mediaUri":"https://example.test/current", "positionMs":u64::MAX, "durationMs":2}),
        json!({"mediaUri":"https://example.test/current", "positionMs":1, "durationMs":2, "action":"stop"}),
    ] {
        assert!(
            bridge
                .publish_text("DLNA_POSITION_REPORTED", payload.to_string())
                .await
                .is_err()
        );
        assert_eq!(state.dlna.state.read().await.current_position_ms, 0);
    }
    core.shutdown().await;
}

#[tokio::test]
async fn stale_players_cannot_change_the_current_media_and_seek_acknowledgements_match_targets() {
    let (_directory, core) = fixture();
    let state = core.runtime_state().core;
    let bridge = core.host_events().unwrap();
    {
        let mut renderer = state.dlna.state.write().await;
        renderer.media_uri = "https://example.test/current".into();
        renderer.playback_state = DlnaPlaybackState::Playing;
        renderer.current_position_ms = 7;
        renderer.seek_target_ms = Some(42);
        let pending = PendingCastRequest {
            sender_ip: "sender".into(),
            sender_name: "Fixture".into(),
            media_uri: "https://example.test/next".into(),
            media_title: "Next".into(),
            media_type: DlnaMediaType::VIDEO,
            album_art_uri: String::new(),
        };
        renderer.raw_pending_cast_request = Some(pending.clone());
        renderer.pending_cast_request = Some(pending);
        renderer.pending_play_queued = true;
    }
    for (kind, mut payload) in [
        (
            "DLNA_POSITION_REPORTED",
            json!({"positionMs":99,"durationMs":100}),
        ),
        ("DLNA_PLAYBACK_REPORTED", json!({"state":"PausedPlayback"})),
        ("DLNA_SEEK_CONSUMED_REPORTED", json!({"positionMs":42})),
        ("DLNA_STOPPED_REPORTED", json!({})),
    ] {
        payload["mediaUri"] = json!("https://example.test/old");
        bridge
            .publish_text(kind, payload.to_string())
            .await
            .unwrap();
        let renderer = state.dlna.state.read().await;
        assert_eq!(renderer.current_position_ms, 7);
        assert_eq!(renderer.media_uri, "https://example.test/current");
        assert_eq!(renderer.playback_state, DlnaPlaybackState::Playing);
        assert_eq!(renderer.seek_target_ms, Some(42));
    }
    bridge
        .publish_text(
            "DLNA_SEEK_CONSUMED_REPORTED",
            json!({"mediaUri":"https://example.test/current","positionMs":41}).to_string(),
        )
        .await
        .unwrap();
    assert_eq!(state.dlna.state.read().await.seek_target_ms, Some(42));
    bridge
        .publish_text(
            "DLNA_SEEK_CONSUMED_REPORTED",
            json!({"mediaUri":"https://example.test/current","positionMs":42}).to_string(),
        )
        .await
        .unwrap();
    assert_eq!(state.dlna.state.read().await.seek_target_ms, None);
    bridge
        .publish_text(
            "DLNA_PLAYBACK_REPORTED",
            json!({"mediaUri":"https://example.test/current","state":"PausedPlayback"}).to_string(),
        )
        .await
        .unwrap();
    assert_eq!(
        state.dlna.state.read().await.playback_state,
        DlnaPlaybackState::PausedPlayback
    );
    bridge
        .publish_text(
            "DLNA_STOPPED_REPORTED",
            json!({"mediaUri":"https://example.test/current"}).to_string(),
        )
        .await
        .unwrap();
    let renderer = state.dlna.state.read().await;
    assert!(renderer.media_uri.is_empty());
    assert_eq!(renderer.playback_state, DlnaPlaybackState::NoMediaPresent);
    assert!(renderer.pending_cast_request.is_some());
    assert!(renderer.raw_pending_cast_request.is_some());
    assert!(renderer.pending_play_queued);
    drop(renderer);
    core.shutdown().await;
}

#[tokio::test]
async fn reports_waiting_on_state_cannot_commit_after_the_host_is_replaced() {
    let (_directory, core) = fixture();
    let state = core.runtime_state().core;
    let original = core.host_events().unwrap();
    let mut locked = state.dlna.state.write().await;
    locked.media_uri = "https://example.test/current".into();
    let mut report = Box::pin(
        original.publish_text(
            "DLNA_POSITION_REPORTED",
            json!({
                "mediaUri":"https://example.test/current", "positionMs":99, "durationMs":100,
            })
            .to_string(),
        ),
    );
    std::future::poll_fn(|context| {
        assert!(std::future::Future::poll(report.as_mut(), context).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    let _replacement = core.host_events().unwrap();
    drop(locked);
    report.await.unwrap();
    assert_eq!(state.dlna.state.read().await.current_position_ms, 0);
    core.shutdown().await;
}
