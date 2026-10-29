use super::*;

#[test]
fn starts_increment_counts_and_position_saves_preserve_them() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    db.media_play_history_record(crate::enums::DataType::Video, "video", 3_000_000_123)
        .unwrap();
    crate::media::play_history::save(&db, crate::enums::DataType::Video, "video", 456).unwrap();
    let row = fixtures::history_row(&db, "video").unwrap();
    assert_eq!(row.position_ms, 456);
    assert_eq!(row.play_count, 1);
    db.media_play_history_record(crate::enums::DataType::Video, "video", 0)
        .unwrap();
    assert_eq!(fixtures::history_row(&db, "video").unwrap().play_count, 2);
}

#[path = "../../library/fixtures.rs"]
mod fixtures;

#[test]
fn normalized_audio_reads_live_catalog_metadata_and_keeps_video_history() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let mut track = crate::media::audio::queue::AudioTrack::from_path_stem("/music/one.mp3");
    track.title = "Original".into();
    fixtures::register_native_audio(&db, &track);
    let playlist = crate::media::audio::queue::create_playlist(&db, "Saved").unwrap();
    crate::media::audio::queue::enqueue(&db, std::slice::from_ref(&track), false).unwrap();
    crate::media::audio::queue::add_playlist_items(&db, &playlist.id, std::slice::from_ref(&track))
        .unwrap();
    crate::media::audio::queue::on_playing(&db, &track.path).unwrap();
    db.media_play_history_record(crate::enums::DataType::Video, "video", 987)
        .unwrap();
    track.title = "Updated".into();
    fixtures::register_native_audio(&db, &track);
    assert_eq!(
        crate::db::audio_queue::all_queue_items(&db).unwrap()[0].title,
        "Updated"
    );
    assert_eq!(
        crate::db::audio_queue::playlist_items(&db, &playlist.id).unwrap()[0].title,
        "Updated"
    );
    assert_eq!(
        crate::media::audio::overview::native(&db)
            .unwrap()
            .recent_items[0]
            .title,
        "Updated"
    );
    crate::db::audio_queue::remove_history(&db, &[track.path.clone()]).unwrap();
    assert!(fixtures::history_row(&db, "video").is_some());
    for table in [
        "audio_queue_items",
        "audio_playlist_items",
        "media_play_history",
    ] {
        let columns = db.with_conn(|conn| {
            conn.prepare(&format!("PRAGMA table_info({table})"))
                .unwrap()
                .query_map([], |row| row.get::<_, String>(1))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap()
        });
        assert!(columns.iter().all(|column| !matches!(
            column.as_str(),
            "path" | "audio_path" | "title" | "artist" | "duration_ms"
        )));
    }
}

#[test]
fn unidentified_audio_plays_without_persisting_references_or_history() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let track = crate::media::audio::queue::AudioTrack::from_path_stem("content://external/stream");
    let playlist = crate::media::audio::queue::create_playlist(&db, "Saved").unwrap();
    assert_eq!(
        crate::media::audio::queue::add_playlist_items(
            &db,
            &playlist.id,
            std::slice::from_ref(&track)
        )
        .unwrap(),
        0
    );
    let playback = crate::media::audio::playback::prepare_track(&db, &track).unwrap();
    crate::media::audio::playback::loaded(&db, &track.path, playback.revision).unwrap();
    assert!(crate::media::audio::playback::started(&db, &track, playback.revision).unwrap());
    assert_eq!(
        crate::media::audio::playback::snapshot(&db).unwrap().path,
        track.path
    );
    db.with_conn(|conn| {
        for table in [
            "audio_queue_items",
            "audio_playlist_items",
            "media_play_history",
        ] {
            assert_eq!(
                conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row
                    .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
    });
}

#[test]
fn platform_system_ids_and_uri_aliases_are_used_without_persisting_metadata() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    db.use_platform_audio_catalog().unwrap();
    let uri = "content://media/external/audio/media/42";
    db.cache_platform_audio(
        &[crate::db::audio_catalog::AudioMedia {
            file_id: Some("42".into()),
            aliases: vec![uri.into()],
            track: crate::media::audio::queue::AudioTrack::from_path_stem("/music/one.mp3"),
        }],
        true,
    )
    .unwrap();
    let track = crate::media::audio::queue::AudioTrack::from_path_stem(uri);
    crate::media::audio::queue::enqueue(&db, std::slice::from_ref(&track), false).unwrap();
    let playback = crate::media::audio::playback::prepare_track(&db, &track).unwrap();
    crate::media::audio::playback::loaded(&db, uri, playback.revision).unwrap();
    crate::media::audio::playback::started(&db, &track, playback.revision).unwrap();
    assert!(
        crate::media::audio::playback::report(&db, uri, playback.revision, 3_000_000_123).unwrap()
    );
    let row = fixtures::history_row(&db, "42").unwrap();
    assert_eq!(row.play_count, 1);
    assert_eq!(row.position_ms, 3_000_000_123);
    db.with_conn(|conn| {
        assert_eq!(
            conn.query_row("SELECT file_id FROM audio_queue_items", [], |row| row
                .get::<_, String>(0))
                .unwrap(),
            "42"
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM file_catalog", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    });
}

#[test]
fn current_system_media_is_restored_after_the_platform_catalog_resolves_it() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    db.use_platform_audio_catalog().unwrap();
    db.audio()
        .update(|state| {
            state.current_file_id = Some("42".into());
            Ok(())
        })
        .unwrap();
    assert!(
        crate::media::audio::playback::snapshot(&db)
            .unwrap()
            .path
            .is_empty()
    );
    db.cache_platform_audio(
        &[crate::db::audio_catalog::AudioMedia {
            file_id: Some("42".into()),
            aliases: Vec::new(),
            track: crate::media::audio::queue::AudioTrack::from_path_stem("/music/one.mp3"),
        }],
        true,
    )
    .unwrap();
    assert_eq!(
        crate::media::audio::playback::snapshot(&db).unwrap().path,
        "/music/one.mp3"
    );
}

#[test]
fn moving_audio_keeps_its_queue_playlist_history_and_current_reference() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let track = crate::media::audio::queue::AudioTrack::from_path_stem("/music/one.mp3");
    fixtures::register_native_audio(&db, &track);
    let id = db
        .media_file_by_path(crate::enums::DataType::Audio, &track.path)
        .unwrap()
        .unwrap()
        .id;
    let playlist = crate::media::audio::queue::create_playlist(&db, "Saved").unwrap();
    crate::media::audio::queue::enqueue(&db, std::slice::from_ref(&track), false).unwrap();
    crate::media::audio::queue::add_playlist_items(&db, &playlist.id, std::slice::from_ref(&track))
        .unwrap();
    crate::media::audio::queue::on_playing(&db, &track.path).unwrap();
    db.media_play_history_record(crate::enums::DataType::Audio, &id, 123)
        .unwrap();
    crate::media::actions::cleanup(
        &db,
        crate::enums::DataType::Audio,
        crate::media::actions::Action::Move,
        &[crate::media::actions::Item {
            id: id.clone(),
            path: track.path,
            destination_path: "/moved/one.mp3".into(),
        }],
    )
    .unwrap();
    assert_eq!(
        crate::db::audio_queue::all_queue_items(&db).unwrap()[0].path,
        "/moved/one.mp3"
    );
    assert_eq!(
        crate::db::audio_queue::playlist_items(&db, &playlist.id).unwrap()[0].audio_path,
        "/moved/one.mp3"
    );
    assert_eq!(
        crate::media::audio::queue::get_audio_current(&db).unwrap(),
        "/moved/one.mp3"
    );
    assert_eq!(fixtures::history_row(&db, &id).unwrap().position_ms, 123);
    db.forget_media_file("/moved/one.mp3").unwrap();
    assert!(
        crate::db::audio_queue::all_queue_items(&db)
            .unwrap()
            .is_empty()
    );
    assert!(
        crate::db::audio_queue::playlist_items(&db, &playlist.id)
            .unwrap()
            .is_empty()
    );
    assert!(
        crate::media::audio::queue::get_audio_current(&db)
            .unwrap()
            .is_empty()
    );
    assert!(fixtures::history_row(&db, &id).is_some());
}

#[test]
fn aliases_of_one_system_media_are_deduplicated_in_queue_and_playlist() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    db.use_platform_audio_catalog().unwrap();
    let uri = "content://media/external/audio/media/42";
    let mut media = crate::db::audio_catalog::AudioMedia {
        file_id: Some("42".into()),
        aliases: vec![uri.into()],
        track: crate::media::audio::queue::AudioTrack::from_path_stem("/music/one.mp3"),
    };
    media.track.album_id = "7".into();
    db.cache_platform_audio(&[media], true).unwrap();
    let tracks = [
        crate::media::audio::queue::AudioTrack::from_path_stem(uri),
        crate::media::audio::queue::AudioTrack::from_path_stem("/music/one.mp3"),
    ];
    crate::media::audio::queue::enqueue(&db, &tracks, false).unwrap();
    let playlist = crate::media::audio::queue::create_playlist(&db, "Saved").unwrap();
    assert_eq!(
        crate::media::audio::queue::add_playlist_items(&db, &playlist.id, &tracks).unwrap(),
        1
    );
    assert_eq!(
        crate::db::audio_queue::all_queue_items(&db).unwrap().len(),
        1
    );
    let items = crate::db::audio_queue::playlist_items(&db, &playlist.id).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].album_id, "7");
    db.forget_platform_audio_paths(&[uri.into()]).unwrap();
    crate::media::audio::queue::enqueue(&db, std::slice::from_ref(&tracks[0]), false).unwrap();
    assert_eq!(
        crate::db::audio_queue::all_queue_items(&db).unwrap().len(),
        1
    );
}

#[test]
fn overflowing_play_counts_leave_the_existing_record_unchanged() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    db.media_play_history_record(crate::enums::DataType::Video, "video", 123)
        .unwrap();
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE media_play_history SET play_count=?1 WHERE media_type=2 AND file_id='video'",
            [i64::MAX],
        )
    })
    .unwrap();
    assert!(
        db.media_play_history_record(crate::enums::DataType::Video, "video", 0)
            .is_err()
    );
    let row = fixtures::history_row(&db, "video").unwrap();
    assert_eq!(row.play_count, i64::MAX);
    assert_eq!(row.position_ms, 123);
}

#[test]
fn explicit_media_type_classifies_history_without_a_loaded_audio_catalog() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    db.media_play_history_record(DataType::Audio, "audio", 111)
        .unwrap();
    db.media_play_history_record(DataType::Video, "video", 222)
        .unwrap();
    crate::media::play_history::save(&db, crate::enums::DataType::Video, "video", 333).unwrap();
    let audio = fixtures::history_row(&db, "audio").unwrap();
    let video = fixtures::history_row(&db, "video").unwrap();
    assert_eq!(
        (audio.media_type, audio.position_ms, audio.play_count),
        (1, 111, 1)
    );
    assert_eq!(
        (video.media_type, video.position_ms, video.play_count),
        (2, 333, 1)
    );
    assert!(
        crate::media::play_history::matching(
            &db,
            &crate::utils::search_dsl::parse("type:VIDEO ids:audio")
        )
        .unwrap()
        .is_empty()
    );
    let recent =
        crate::media::play_history::matching(&db, &crate::utils::search_dsl::parse("type:VIDEO"))
            .unwrap();
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].file_id, "video");
    let history = crate::media::audio::overview::history(&db).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].id, "audio");
    assert_eq!(db.audio_reference_ids().unwrap(), vec!["audio"]);
    crate::media::actions::cleanup(
        &db,
        DataType::Video,
        crate::media::actions::Action::Delete,
        &[crate::media::actions::Item {
            id: "audio".into(),
            path: "/owned/video.mp4".into(),
            destination_path: String::new(),
        }],
    )
    .unwrap();
    assert_eq!(fixtures::history_row(&db, "audio"), Some(audio));
}

#[test]
fn history_requires_explicit_types_and_globally_unique_file_ids() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    assert!(
        db.media_play_history_record(DataType::Image, "image", 0)
            .is_err()
    );
    db.media_play_history_record(DataType::Audio, "audio", 111)
        .unwrap();
    let audio = fixtures::history_row(&db, "audio");
    db.with_conn(|conn| {
        for sql in [
            "INSERT INTO media_play_history(file_id,position_ms,play_count,played_at) VALUES('missing',0,0,'now')",
            "INSERT INTO media_play_history(file_id,media_type,position_ms,play_count,played_at) VALUES('invalid',3,0,0,'now')",
            "INSERT INTO media_play_history(file_id,media_type,position_ms,play_count,played_at) VALUES('audio',2,0,0,'now')",
        ] {
            assert!(conn.execute(sql, []).is_err());
        }
    });
    assert!(
        crate::media::play_history::save(&db, crate::enums::DataType::Video, "audio", 999).is_err()
    );
    assert!(
        crate::media::play_history::record(&db, crate::enums::DataType::Video, "audio", 999)
            .is_err()
    );
    assert_eq!(fixtures::history_row(&db, "audio"), audio);
}
