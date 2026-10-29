#[path = "../../library/fixtures.rs"]
mod fixtures;
use fixtures::*;

#[test]
fn replace_queue_items_assigns_dense_sort_orders() {
    let db = test_db("replace_dense");
    register_audio_tracks(&db, &[audio("/a.mp3", "A", 1), audio("/b.mp3", "B", 2)]);
    crate::db::audio_queue::replace_queue_items(
        &db,
        &[
            crate::db::QueueItem {
                file_id: format!("test:{}", "/a.mp3"),
                path: "/a.mp3".into(),
                sort_order: 99,
                title: "A".into(),
                artist: String::new(),
                duration_ms: 1,
            },
            crate::db::QueueItem {
                file_id: format!("test:{}", "/b.mp3"),
                path: "/b.mp3".into(),
                sort_order: -5,
                title: "B".into(),
                artist: String::new(),
                duration_ms: 2,
            },
        ],
    )
    .unwrap();
    let items = crate::db::audio_queue::all_queue_items(&db).unwrap();
    assert_eq!(items[0].sort_order, 0);
    assert_eq!(items[1].sort_order, 1);

    // Empty slice clears the table.
    crate::db::audio_queue::replace_queue_items(&db, &[]).unwrap();
    assert!(
        crate::db::audio_queue::all_queue_items(&db)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn history_records_order_equal_timestamps_by_rowid_and_update_existing_rows() {
    let db = test_db("history_io");
    for (path, played_at) in [
        ("/old.mp3", "2026-01-01T00:00:00.000Z"),
        ("/new.mp3", "2026-01-02T00:00:00.000Z"),
        ("/tie1.mp3", "2026-01-02T00:00:00.000Z"),
        ("/tie2.mp3", "2026-01-02T00:00:00.000Z"),
    ] {
        register_audio(&db, &audio(path, "T", 1));
        db.media_play_history_record(crate::enums::DataType::Audio, &format!("test:{path}"), 0)
            .unwrap();
        db.with_conn(|conn| {
            conn.execute(
                "UPDATE media_play_history SET played_at=?1 WHERE file_id=?2",
                rusqlite::params![played_at, format!("test:{path}")],
            )
        })
        .unwrap();
    }
    let rows = crate::media::play_history::matching(&db, &[]).unwrap();
    assert_eq!(
        rows.iter()
            .map(|row| row.file_id.as_str())
            .collect::<Vec<_>>(),
        [
            "test:/tie2.mp3",
            "test:/tie1.mp3",
            "test:/new.mp3",
            "test:/old.mp3"
        ]
    );
    db.media_play_history_record(crate::enums::DataType::Audio, "test:/old.mp3", 123)
        .unwrap();
    db.with_conn(|conn| conn.execute("UPDATE media_play_history SET played_at='2026-01-03T00:00:00.000Z' WHERE file_id='test:/old.mp3'", [])).unwrap();
    let rows = crate::media::play_history::matching(&db, &[]).unwrap();
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[0].file_id, "test:/old.mp3");
    assert_eq!(rows[0].play_count, 2);
    assert_eq!(rows[0].position_ms, 123);
}

#[test]
fn remove_by_paths_deletes_only_matching_audio_history() {
    let db = test_db("remove_paths_io");
    for path in ["/a.mp3", "/b.mp3"] {
        register_audio(&db, &audio(path, "Track", 1));
        db.media_play_history_record(crate::enums::DataType::Audio, &format!("test:{path}"), 0)
            .unwrap();
    }
    db.media_play_history_record(crate::enums::DataType::Video, "video", 999)
        .unwrap();
    crate::db::audio_queue::remove_history(&db, &["/a.mp3".into(), "/missing.mp3".into()]).unwrap();
    let rows =
        crate::media::play_history::matching(&db, &crate::utils::search_dsl::parse("type:AUDIO"))
            .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].file_id, "test:/b.mp3");
    assert_eq!(history_row(&db, "video").unwrap().position_ms, 999);
}

#[test]
fn playlist_item_counts_and_bulk_delete() {
    let db = test_db("pl_counts");
    register_audio_tracks(
        &db,
        &[
            audio("/1.mp3", "One", 0),
            audio("/2.mp3", "Two", 0),
            audio("/3.mp3", "Three", 0),
        ],
    );
    let item = |pl: &str, path: &str, sort: i64| crate::db::PlaylistItem {
        file_id: format!("test:{path}"),
        id: format!("{pl}-{path}"),
        playlist_id: pl.into(),
        audio_path: path.into(),
        title: String::new(),
        artist: String::new(),
        duration_ms: 0,
        album_id: String::new(),
        sort_order: sort,
        added_at: String::new(),
    };
    crate::db::audio_queue::insert_playlist_item(&db, &item("p1", "/1.mp3", 0)).unwrap();
    crate::db::audio_queue::insert_playlist_item(&db, &item("p1", "/2.mp3", 1)).unwrap();
    crate::db::audio_queue::insert_playlist_item(&db, &item("p2", "/3.mp3", 0)).unwrap();
    let counts = crate::db::audio_queue::playlist_item_counts(&db).unwrap();
    assert_eq!(counts.get("p1"), Some(&2));
    assert_eq!(counts.get("p2"), Some(&1));
    assert!(!counts.contains_key("pX"));

    // Cascade delete by path across playlists.
    crate::db::audio_queue::remove_playlist_items_by_paths(&db, &["/1.mp3".to_string()]).unwrap();
    assert_eq!(
        crate::db::audio_queue::playlist_items(&db, "p1")
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn queue_path_lookup_does_not_require_a_play_history_row() {
    let db = test_db("queue_path_lookup");
    let track = audio("/queued.mp3", "Queued", 123);
    register_audio(&db, &track);
    crate::media::audio::queue::enqueue(&db, std::slice::from_ref(&track), false).unwrap();
    assert!(
        crate::media::audio::overview::history(&db)
            .unwrap()
            .is_empty()
    );
    let row = crate::db::audio_queue::queue_item_by_path(&db, &track.path)
        .unwrap()
        .unwrap();
    assert_eq!(row.path, track.path);
    assert_eq!(row.title, "Queued");
    assert_eq!(row.duration_ms, 123);
    assert!(
        crate::db::audio_queue::queue_item_by_path(&db, "/missing.mp3")
            .unwrap()
            .is_none()
    );
}
