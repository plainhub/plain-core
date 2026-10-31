use plain_rs::media::audio::{playback, queue};

#[tokio::test]
async fn native_host_lifetime_restores_position_and_invalidates_reports_once_per_generation() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state().core;
    let track = queue::AudioTrack::from_path_stem("/owned/track.mp3");
    state.db.with_conn(|conn| conn.execute(
        "INSERT INTO platform_audio_media(file_id,path,title,artist,duration_ms,album_id) VALUES('owned-track',?1,'Track','',0,'')",
        [&track.path],
    )).unwrap();
    queue::enqueue(&state.db, std::slice::from_ref(&track), false).unwrap();
    playback::prepare_track(&state.db, &track).unwrap();
    let previous = playback::seek(&state.db, 1234).unwrap();
    let (first, requests) = state.host.connect();
    let restored = state
        .audio
        .run_playback(|db, _| playback::snapshot(db))
        .await
        .unwrap();
    assert_eq!(restored.path, track.path);
    assert_eq!(restored.position_ms, 1234);
    assert!(restored.revision > previous.revision);
    assert!(!playback::report(&state.db, &track.path, previous.revision, 0).unwrap());
    assert!(requests.is_empty());
    let again = state
        .audio
        .run_playback(|db, _| playback::snapshot(db))
        .await
        .unwrap();
    assert_eq!(again, restored);
    let (second, requests) = state.host.connect();
    assert!(second > first);
    let replacement = state
        .audio
        .run_playback(|db, _| playback::snapshot(db))
        .await
        .unwrap();
    assert_eq!(replacement.position_ms, 1234);
    assert!(replacement.revision > restored.revision);
    assert!(!playback::report(&state.db, &track.path, restored.revision, 0).unwrap());
    assert!(requests.is_empty());
    let retired = core
        .native_graphql()
        .unwrap()
        .execute(async_graphql::Request::new(
            "mutation { invalidateAudioEngine { revision } }",
        ))
        .await;
    assert!(!retired.errors.is_empty());
    core.stop_core().await;
}
