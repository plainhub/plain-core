use super::*;
use crate::assembly::server::ContentServer;
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn completion_fact_dispatches_once_and_never_rebroadcasts_the_platform_event() {
    let dir = tempfile::tempdir().unwrap();
    let server = ContentServer::start(
        dir.path(),
        &plain_rs::utils::base64::base64_encode(&[9; 32]),
        Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap()),
    )
    .unwrap();
    let state = server.runtime_state();
    state.db.restore_audio_current().unwrap();
    let (generation, mut requests) = state.host.connect();
    state
        .audio
        .run_playback(|db, _| playback::snapshot(db))
        .await
        .unwrap();
    let playback = playback::prepare_track(&state.db, &AudioTrack::from_path_stem("one")).unwrap();
    let host = state.host.clone();
    let responder = tokio::spawn(async move {
        let request = requests.recv().await.unwrap();
        assert_eq!(request["method"], "audioEngineCommand");
        assert_eq!(request["params"]["action"], "PAUSE");
        let mut report = request["params"]["playback"].clone();
        report["loaded"] = json!(false);
        host.reply(generation, json!({"id":request["id"],"result":report}))
            .unwrap();
        requests
    });
    let mut events = state.events.subscribe();
    let payload = json!({"path":"one","revision":playback.revision}).to_string();
    assert!(
        accept(&state, generation, "AUDIO_COMPLETED_REPORTED", &payload)
            .await
            .unwrap()
    );
    assert_eq!(events.try_recv().unwrap().event_type, "CONTENT_CHANGED");
    assert!(events.try_recv().is_err());
    let mut requests = responder.await.unwrap();
    assert!(
        accept(&state, generation, "AUDIO_COMPLETED_REPORTED", &payload)
            .await
            .unwrap()
    );
    assert!(events.try_recv().is_err());
    assert!(requests.try_recv().is_err());
    assert!(
        accept(
            &state,
            generation,
            "AUDIO_COMPLETED_REPORTED",
            r#"{"path":"one","revision":-1}"#
        )
        .await
        .is_err()
    );
    assert!(
        accept(
            &state,
            generation,
            "AUDIO_COMPLETED_REPORTED",
            r#"{"path":"","revision":1}"#
        )
        .await
        .is_err()
    );
    state.host.disconnect(generation);
    assert!(
        accept(&state, generation, "AUDIO_COMPLETED_REPORTED", &payload)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn an_acknowledged_start_notifies_the_library_once_without_echoing_the_sdk_fact() {
    let dir = tempfile::tempdir().unwrap();
    let server = ContentServer::start(
        dir.path(),
        &plain_rs::utils::base64::base64_encode(&[9; 32]),
        Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("prefs.json")).unwrap()),
    )
    .unwrap();
    let state = server.runtime_state();
    state.db.restore_audio_current().unwrap();
    state.db.use_platform_audio_catalog().unwrap();
    state
        .db
        .cache_platform_audio(
            &[plain_rs::db::audio_catalog::AudioMedia {
                file_id: Some("track".into()),
                aliases: vec![],
                track: AudioTrack::from_path_stem("one"),
            }],
            false,
        )
        .unwrap();
    let playback = playback::prepare_track(&state.db, &AudioTrack::from_path_stem("one")).unwrap();
    playback::loaded(&state.db, "one", playback.revision).unwrap();
    let (generation, mut requests) = state.host.connect();
    let mut events = state.events.subscribe();
    let payload = json!({"path":"one","revision":playback.revision}).to_string();
    accept(&state, generation, "AUDIO_STARTED_REPORTED", &payload)
        .await
        .unwrap();
    let callback = requests.try_recv().unwrap();
    assert_eq!(callback["method"], "audioLibraryRefresh");
    state
        .host
        .reply(generation, json!({"id":callback["id"],"result":null}))
        .unwrap();
    assert!(events.try_recv().is_err());
    accept(&state, generation, "AUDIO_STARTED_REPORTED", &payload)
        .await
        .unwrap();
    assert!(events.try_recv().is_err());
    assert!(requests.try_recv().is_err());
}
