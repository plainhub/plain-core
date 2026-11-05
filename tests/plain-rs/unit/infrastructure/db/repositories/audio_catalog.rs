use super::*;

#[test]
fn selected_audio_follows_its_file_identity_after_the_catalog_path_changes() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    db.use_platform_audio_catalog().unwrap();
    let row = |path: &str| AudioMedia {
        file_id: Some("track".into()),
        aliases: Vec::new(),
        track: AudioTrack::from_path_stem(path),
    };
    db.cache_platform_audio(&[row("/previous-container/song.mp3")], false)
        .unwrap();
    let track = crate::features::media::audio::queue::track_by_id(&db, "track").unwrap();
    crate::features::media::audio::playback::prepare_track(&db, &track).unwrap();
    let before = crate::features::media::audio::playback::seek(&db, 1234).unwrap();
    db.cache_platform_audio(&[row("/current-container/song.mp3")], false)
        .unwrap();

    let after = crate::features::media::audio::playback::snapshot(&db).unwrap();
    assert_eq!(after.path, "/current-container/song.mp3");
    assert_eq!(after.position_ms, 1234);
    assert!(after.revision > before.revision);
    assert_eq!(db.audio_current_file_id().as_deref(), Some("track"));
    assert!(
        !crate::features::media::audio::playback::report(&db, &before.path, before.revision, 0)
            .unwrap()
    );
}

#[test]
fn parsed_metadata_updates_only_existing_platform_rows_and_preserves_identity_and_aliases() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let mut track = AudioTrack::from_path_stem("content://media/external/audio/media/7");
    track.file_id = Some("7".into());
    track.title = "Parsed Title".into();
    track.artist = "Parsed Artist".into();
    track.duration_ms = 1234;
    assert_eq!(db.update_platform_audio_metadata(&track).unwrap(), 0);
    db.use_platform_audio_catalog().unwrap();
    db.cache_platform_audio(
        &[AudioMedia {
            file_id: Some("7".into()),
            aliases: vec!["content://documents/audio/7".into()],
            track: AudioTrack::from_path_stem(&track.path),
        }],
        false,
    )
    .unwrap();
    assert_eq!(db.update_platform_audio_metadata(&track).unwrap(), 1);
    for path in [&track.path[..], "content://documents/audio/7"] {
        let actual = db
            .with_conn(|conn| crate::features::media::audio::queue::track_by_path_conn(conn, path))
            .unwrap()
            .unwrap();
        assert_eq!(actual.file_id, track.file_id);
        assert_eq!(actual.path, track.path);
        assert_eq!(actual.title, track.title);
        assert_eq!(actual.artist, track.artist);
        assert_eq!(actual.duration_ms, track.duration_ms);
    }
    assert_eq!(
        db.with_conn(
            |conn| conn.query_row("SELECT COUNT(*) FROM file_catalog", [], |row| row
                .get::<_, i64>(0))
        )
        .unwrap(),
        0
    );
}
