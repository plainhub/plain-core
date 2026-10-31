use super::*;

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
            .with_conn(|conn| crate::media::audio::queue::track_by_path_conn(conn, path))
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
