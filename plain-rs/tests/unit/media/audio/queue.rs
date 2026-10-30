use super::*;
#[path = "../../library/fixtures.rs"]
mod fixtures;
use fixtures::*;

#[test]
fn explicit_id_selection_preserves_order_and_rejects_empty_members() {
    assert_eq!(
        selection_ids("ids:b,a,b").unwrap(),
        Some(vec!["b".into(), "a".into(), "b".into()])
    );
    for query in ["ids:", "ids:a,", "ids:,a", "ids:a,,b"] {
        assert!(selection_ids(query).is_err(), "{query}");
    }
    for query in [
        "",
        "text:music",
        "ids:a artist:singer",
        "NOT ids:a",
        "ids:>a",
    ] {
        assert_eq!(selection_ids(query).unwrap(), None, "{query}");
    }
}

#[test]
fn owned_queue_rows_keep_their_string_allocations_when_projected() {
    let row = QueueItem {
        file_id: "synthetic-audio".into(),
        path: "/synthetic/track.mp3".into(),
        title: "Synthetic title".into(),
        artist: "Synthetic artist".into(),
        duration_ms: 1234,
        sort_order: 7,
    };
    let allocations = [
        row.file_id.as_ptr(),
        row.path.as_ptr(),
        row.title.as_ptr(),
        row.artist.as_ptr(),
    ];
    let track = AudioTrack::from(row);
    assert_eq!(
        [
            track.file_id.as_ref().unwrap().as_ptr(),
            track.path.as_ptr(),
            track.title.as_ptr(),
            track.artist.as_ptr()
        ],
        allocations
    );
    assert_eq!(track.file_id.as_deref(), Some("synthetic-audio"));
    assert_eq!(track.path, "/synthetic/track.mp3");
    assert_eq!(track.title, "Synthetic title");
    assert_eq!(track.artist, "Synthetic artist");
    assert_eq!(track.duration_ms, 1234);
    assert!(track.album_id.is_empty());
}

#[test]
fn enqueue_appends_moves_and_never_duplicates() {
    let db = test_db("enqueue");
    indexed_enqueue(
        &db,
        &[audio("/tq1/a.mp3", "A", 1), audio("/tq1/b.mp3", "B", 2)],
        false,
    )
    .unwrap();
    indexed_enqueue(
        &db,
        &[audio("/tq1/b.mp3", "B", 2), audio("/tq1/c.mp3", "C", 3)],
        false,
    )
    .unwrap();
    let queued = crate::db::audio_queue::all_queue_items(&db).unwrap();
    assert_eq!(
        queued.iter().map(|q| q.path.as_str()).collect::<Vec<_>>(),
        ["/tq1/a.mp3", "/tq1/b.mp3", "/tq1/c.mp3"]
    );
    indexed_enqueue(&db, &[audio("/tq1/z.mp3", "Z", 9)], true).unwrap();
    let queued = crate::db::audio_queue::all_queue_items(&db).unwrap();
    assert_eq!(queued[0].path, "/tq1/z.mp3");
    for (i, q) in queued.iter().enumerate() {
        assert_eq!(q.sort_order, i as i64);
    }
}

#[test]
fn remove_paths_prunes_queue_history_playlist_items_and_current() {
    let db = test_db("remove_paths");
    indexed_enqueue(
        &db,
        &[audio("/tq3/a.mp3", "A", 1), audio("/tq3/b.mp3", "B", 2)],
        false,
    )
    .unwrap();
    let pl = create_playlist(&db, "PL").unwrap();
    indexed_add_playlist_items(
        &db,
        &pl.id,
        &[audio("/tq3/a.mp3", "A", 1), audio("/tq3/keep.mp3", "K", 5)],
    )
    .unwrap();
    indexed_on_playing(&db, "/tq3/stay.mp3", "S", "A", 9).unwrap();
    indexed_on_playing(&db, "/tq3/a.mp3", "A", "A", 1).unwrap();

    remove_paths(&db, &["/tq3/a.mp3".to_string()]).unwrap();

    assert!(
        crate::db::audio_queue::queue_item_by_path(&db, "/tq3/a.mp3")
            .unwrap()
            .is_none()
    );
    assert!(history_row(&db, "test:/tq3/a.mp3").is_none());
    assert_eq!(playlist_item_count(&db, &pl.id).unwrap(), 1);
    assert_eq!(get_audio_current(&db).unwrap(), "");
    assert!(
        crate::db::audio_queue::queue_item_by_path(&db, "/tq3/b.mp3")
            .unwrap()
            .is_some()
    );
}

#[test]
fn playlist_crud_items_and_counts() {
    let db = test_db("playlist_crud");
    let pl = create_playlist(&db, "Roadtrip").unwrap();
    assert_eq!(pl.name, "Roadtrip");
    assert_eq!(
        indexed_add_playlist_items(
            &db,
            &pl.id,
            &[
                audio("/tq5/a.mp3", "A", 1),
                audio("/tq5/b.mp3", "B", 2),
                audio("/tq5/a.mp3", "A-dup", 1),
            ],
        )
        .unwrap(),
        2
    );
    assert_eq!(playlist_item_count(&db, &pl.id).unwrap(), 2);
    let page = playlist_items_page(&db, &pl.id, 1, 5, "").unwrap();
    assert_eq!(paths_of(&page), ["/tq5/b.mp3"]);

    let listed = playlists(&db).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].1, 2);
    assert_eq!(listed[0].0.name, "Roadtrip");

    rename_playlist(&db, &pl.id, "Roadtrip 2").unwrap();
    assert_eq!(
        playlist_by_id(&db, &pl.id).unwrap().unwrap().name,
        "Roadtrip 2"
    );

    assert!(rename_playlist(&db, "nope", "X").is_err());

    let other = create_playlist(&db, "Other").unwrap();
    assert_eq!(playlists(&db).unwrap()[0].0.id, other.id);

    remove_playlist_item(&db, &pl.id, "/tq5/a.mp3").unwrap();
    assert_eq!(playlist_item_count(&db, &pl.id).unwrap(), 1);

    delete_playlist(&db, &pl.id).unwrap();
    assert!(playlist_by_id(&db, &pl.id).unwrap().is_none());
    assert!(
        crate::db::audio_queue::playlist_items(&db, &pl.id)
            .unwrap()
            .is_empty()
    );
}
#[test]
fn text_filter_precedes_pagination_on_queue_and_playlist() {
    let db = test_db("text_filter");
    indexed_enqueue(
        &db,
        &[
            audio("/tf/road song.mp3", "Road Song", 1),
            audio("/tf/sea song.mp3", "Sea Song", 2),
            audio("/tf/zzz.mp3", "Zzz", 3),
        ],
        false,
    )
    .unwrap();
    assert_eq!(
        paths_of(&queue_page(&db, 0, 10, "SONG").unwrap()),
        ["/tf/road song.mp3", "/tf/sea song.mp3"]
    );
    assert_eq!(
        paths_of(&queue_page(&db, 1, 1, "song").unwrap()),
        ["/tf/sea song.mp3"]
    );
    assert_eq!(
        paths_of(&queue_page(&db, 0, 10, "/tf/road").unwrap()),
        ["/tf/road song.mp3"]
    );
    assert!(queue_page(&db, 0, 10, "nope").unwrap().is_empty());
    let pl = create_playlist(&db, "mix").unwrap();
    indexed_add_playlist_items(
        &db,
        &pl.id,
        &[
            audio("/tf/road song.mp3", "Road Song", 1),
            audio("/tf/zzz.mp3", "Zzz", 3),
        ],
    )
    .unwrap();
    assert_eq!(
        paths_of(&playlist_items_page(&db, &pl.id, 0, 10, "road").unwrap()),
        ["/tf/road song.mp3"]
    );
    assert_eq!(
        playlist_items_page(&db, &pl.id, 0, 10, "").unwrap().len(),
        2
    );
}

#[test]
fn play_starts_increment_counts_and_trim_old_audio_history() {
    let db = test_db("history_trim");
    indexed_on_playing(&db, "/tq7/a.mp3", "A", "X", 1).unwrap();
    indexed_on_playing(&db, "/tq7/a.mp3", "A2", "X", 2).unwrap();
    let h = history_row(&db, "test:/tq7/a.mp3").unwrap();
    assert_eq!(h.play_count, 2);
    let metadata = track_by_id(&db, "test:/tq7/a.mp3").unwrap();
    assert_eq!(metadata.title, "A2");
    assert_eq!(metadata.duration_ms, 2);
    for i in 0..(crate::db::HISTORY_KEEP * 5 / 4 + 2) {
        indexed_on_playing(&db, &format!("/tq7/x{i}.mp3"), "T", "X", 1).unwrap();
    }
    assert_eq!(
        crate::media::audio::overview::history(&db).unwrap().len(),
        crate::db::HISTORY_KEEP + 2
    );
    let page = crate::media::audio::overview::history(&db).unwrap();
    assert!(page[0].id.starts_with("test:/tq7/x"));
    assert!(
        crate::media::audio::overview::history(&db)
            .unwrap()
            .iter()
            .all(|r| !r.id.ends_with("a.mp3"))
    );
}

#[test]
fn audio_track_from_path_stem() {
    let a = AudioTrack::from_path_stem("/music/My Song.mp3");
    assert_eq!(a.title, "My Song");
    assert_eq!(a.artist, "");
    assert_eq!(a.duration_ms, 0);
    assert_eq!(a.path, "/music/My Song.mp3");
    assert_eq!(AudioTrack::from_path_stem("\\a\\b.mp3").path, "/a/b.mp3");
}

#[test]
fn mode_defaults_to_repeat_and_roundtrips() {
    let dir = tempfile::tempdir().unwrap();
    let prefs = crate::prefs::Prefs::load(&crate::prefs::default_path(dir.path())).unwrap();
    assert_eq!(get_audio_mode(&prefs).unwrap(), "REPEAT");
    save_audio_mode(&prefs, "SHUFFLE").unwrap();
    assert_eq!(get_audio_mode(&prefs).unwrap(), "SHUFFLE");
    save_audio_mode(&prefs, "  REPEAT_ONE ").unwrap();
    assert_eq!(get_audio_mode(&prefs).unwrap(), "REPEAT_ONE");
    let stored: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("prefs.json")).unwrap())
            .unwrap();
    assert_eq!(stored["audio_play_mode"], "REPEAT_ONE");
    prefs.set("audio_play_mode", "SHUFFLE").unwrap();
    assert_eq!(get_audio_mode(&prefs).unwrap(), "SHUFFLE");
    prefs.set("audio_play_mode", 99).unwrap();
    assert!(get_audio_mode(&prefs).is_err());
    assert!(save_audio_mode(&prefs, "unsupported").is_err());
}
#[test]
fn state_survives_reopen() {
    let dir = std::env::temp_dir().join(format!("plain-rs-library-reopen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("plain.db");
    {
        let db = crate::db::Db::open(&path).unwrap();
        let pl = create_playlist(&db, "Persist").unwrap();
        indexed_add_playlist_items(&db, &pl.id, &[audio("/p/a.mp3", "A", 1)]).unwrap();
        indexed_on_playing(&db, "/p/a.mp3", "A", "X", 1).unwrap();
    }
    let db = crate::db::Db::open(&path).unwrap();
    fixtures::register_audio(&db, &audio("/p/a.mp3", "A", 1));
    let listed = playlists(&db).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].1, 1);
    assert_eq!(
        crate::media::audio::overview::history(&db).unwrap().len(),
        1
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn milliseconds_survive_restart_without_persisting_album_snapshots() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audio.db");
    let db = crate::db::Db::open(&path).unwrap();
    let pl = create_playlist(&db, "Precise").unwrap();
    let mut item = audio("/precise.mp3", "Precise", 117_123);
    item.album_id = "album-42".into();
    fixtures::register_native_audio(&db, &item);
    add_playlist_items(&db, &pl.id, std::slice::from_ref(&item)).unwrap();
    enqueue(&db, std::slice::from_ref(&item), false).unwrap();
    on_playing(&db, &item.path).unwrap();
    drop(db);
    let db = crate::db::Db::open(&path).unwrap();
    assert_eq!(
        crate::db::audio_queue::all_queue_items(&db).unwrap()[0].duration_ms,
        117_123
    );
    let saved = playlist_items_page(&db, &pl.id, 0, 10, "")
        .unwrap()
        .remove(0);
    assert_eq!(saved.duration_ms, 117_123);
    assert_eq!(saved.album_id, "");
    assert_eq!(
        db.media_file_by_path(crate::enums::DataType::Audio, "/precise.mp3")
            .unwrap()
            .unwrap()
            .duration_ms,
        117_123
    );
}

#[test]
fn duplicate_enqueue_uses_last_snapshot_and_preserves_other_tracks() {
    let db = test_db("queue_duplicates");
    indexed_enqueue(&db, &[audio("/old", "Old", 1)], false).unwrap();
    indexed_enqueue(
        &db,
        &[audio("/new", "First", 17), audio("/new", "Latest", 123)],
        true,
    )
    .unwrap();
    let rows = crate::db::audio_queue::all_queue_items(&db).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].title, "Latest");
    assert_eq!(rows[0].duration_ms, 123);
    assert_eq!(rows[1].path, "/old");
    assert!(reorder_queued(&db, &["test:/old".into(), "test:/old".into()]).is_err());
    assert_eq!(
        crate::db::audio_queue::all_queue_items(&db).unwrap().len(),
        2
    );
}

#[test]
fn failed_queue_and_playlist_batch_writes_roll_back() {
    let db = test_db("rollback_batch");
    indexed_enqueue(&db, &[audio("/old", "Old", 1)], false).unwrap();
    db.with_conn(|c|c.execute_batch("CREATE TRIGGER reject_queue BEFORE INSERT ON audio_queue_items WHEN NEW.file_id='test:/bad' BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;")).unwrap();
    assert!(
        indexed_enqueue(
            &db,
            &[audio("/ok", "Ok", 2), audio("/bad", "Bad", 3)],
            false
        )
        .is_err()
    );
    assert_eq!(
        crate::db::audio_queue::all_queue_items(&db)
            .unwrap()
            .iter()
            .map(|r| r.path.as_str())
            .collect::<Vec<_>>(),
        vec!["/old"]
    );
    let pl = create_playlist(&db, "List").unwrap();
    db.with_conn(|c|c.execute_batch("CREATE TRIGGER reject_playlist BEFORE INSERT ON audio_playlist_items WHEN NEW.file_id='test:/bad' BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;")).unwrap();
    assert!(
        indexed_add_playlist_items(
            &db,
            &pl.id,
            &[audio("/ok", "Ok", 2), audio("/bad", "Bad", 3)]
        )
        .is_err()
    );
    assert_eq!(playlist_item_count(&db, &pl.id).unwrap(), 0);
    assert!(indexed_add_playlist_items(&db, "unknown", &[audio("/ok", "Ok", 2)]).is_err());
}

#[test]
fn concurrent_queue_and_history_writes_do_not_lose_updates() {
    let db = test_db("concurrent_audio");
    let workers: Vec<_> = (0..16)
        .map(|i| {
            let db = db.clone();
            std::thread::spawn(move || {
                indexed_enqueue(&db, &[audio(&format!("/{i}"), "Track", 123)], false).unwrap();
                indexed_on_playing(&db, "/shared", "Track", "Artist", 123).unwrap();
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
    assert_eq!(
        crate::db::audio_queue::all_queue_items(&db).unwrap().len(),
        16
    );
    assert_eq!(history_row(&db, "test:/shared").unwrap().play_count, 16);
}

#[test]
fn failed_media_cleanup_rolls_back_every_audio_table() {
    let db = test_db("cleanup_failure");
    let pl = create_playlist(&db, "List").unwrap();
    indexed_add_playlist_items(&db, &pl.id, &[audio("/one", "One", 123)]).unwrap();
    indexed_on_playing(&db, "/one", "One", "Artist", 123).unwrap();
    indexed_enqueue(&db, &[audio("/one", "One", 123)], false).unwrap();
    db.with_conn(|c|c.execute_batch("CREATE TRIGGER reject_history_delete BEFORE DELETE ON media_play_history BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;")).unwrap();
    assert!(remove_paths(&db, &["/one".into()]).is_err());
    assert_eq!(
        crate::db::audio_queue::all_queue_items(&db).unwrap().len(),
        1
    );
    assert_eq!(playlist_item_count(&db, &pl.id).unwrap(), 1);
    assert_eq!(get_audio_current(&db).unwrap(), "/one");
    db.with_conn(|c| c.execute_batch("DROP TRIGGER reject_history_delete;"))
        .unwrap();
    remove_paths(&db, &["/one".into()]).unwrap();
    assert_eq!(playlist_item_count(&db, &pl.id).unwrap(), 0);
    assert!(
        crate::media::audio::overview::history(&db)
            .unwrap()
            .is_empty()
    );
    assert!(get_audio_current(&db).unwrap().is_empty());
}

#[test]
fn materialized_queue_exceeds_old_limits_and_keeps_existing_positions() {
    let db = test_db("unlimited");
    let tracks: Vec<_> = (0..1205)
        .map(|i| audio(&format!("/track/{i}"), &format!("Track {i}"), i))
        .collect();
    indexed_enqueue(&db, &tracks, false).unwrap();
    assert_eq!(queue_total(&db).unwrap(), 1205);
    let before = queue_page(&db, 0, 1205, "").unwrap();
    enqueue(&db, &[tracks[0].clone(), tracks[1000].clone()], false).unwrap();
    assert_eq!(queue_page(&db, 0, 1205, "").unwrap(), before);
    assert_eq!(queue_page(&db, 1000, 200, "").unwrap().len(), 200);
    assert_eq!(queue_page(&db, 1200, 200, "").unwrap().len(), 5);
}

#[test]
fn prefix_reorder_preserves_tail_and_current_track_and_navigation() {
    let db = test_db("prefix");
    let tracks: Vec<_> = (0..5)
        .map(|i| audio(&format!("/track/{i}"), "Track", 1))
        .collect();
    indexed_enqueue(&db, &tracks, false).unwrap();
    save_audio_current(&db, &tracks[1].path).unwrap();
    let ids: Vec<_> = queue_page(&db, 0, 5, "")
        .unwrap()
        .iter()
        .map(|track| track.file_id.clone().unwrap())
        .collect();
    reorder_queued(&db, &[ids[2].clone(), ids[0].clone(), ids[1].clone()]).unwrap();
    assert_eq!(
        queue_page(&db, 0, 5, "")
            .unwrap()
            .into_iter()
            .map(|track| track.file_id.unwrap())
            .collect::<Vec<_>>(),
        [
            ids[2].clone(),
            ids[0].clone(),
            ids[1].clone(),
            ids[3].clone(),
            ids[4].clone()
        ]
    );
    assert_eq!(get_audio_current(&db).unwrap(), tracks[1].path);
    assert_eq!(
        select_next(&db, true, false).unwrap().unwrap().path,
        tracks[3].path
    );
    assert_eq!(
        select_next(&db, false, false).unwrap().unwrap().path,
        tracks[1].path
    );
    let before = queue_page(&db, 0, 5, "").unwrap();
    assert!(reorder_queued(&db, &[ids[0].clone(), ids[0].clone()]).is_err());
    assert!(reorder_queued(&db, &["absent".into()]).is_err());
    assert_eq!(queue_page(&db, 0, 5, "").unwrap(), before);
}

#[test]
fn removing_current_stops_selection_and_playlist_deletion_leaves_queue() {
    let db = test_db("remove_current");
    let tracks = [audio("/one", "One", 1), audio("/two", "Two", 1)];
    indexed_enqueue(&db, &tracks, false).unwrap();
    let playlist = create_playlist(&db, "List").unwrap();
    add_playlist_items(&db, &playlist.id, &tracks).unwrap();
    save_audio_current(&db, "/one").unwrap();
    delete_playlist(&db, &playlist.id).unwrap();
    assert_eq!(queue_total(&db).unwrap(), 2);
    assert_eq!(get_audio_current(&db).unwrap(), "/one");
    assert!(!remove_queued(&db, "absent").unwrap());
    assert!(remove_queued(&db, "test:/one").unwrap());
    assert_eq!(get_audio_current(&db).unwrap(), "");
    assert_eq!(db.audio_current_file_id(), None);
    assert_eq!(queue_total(&db).unwrap(), 1);
    save_audio_current(&db, "/two").unwrap();
    clear_queue(&db).unwrap();
    assert_eq!(queue_total(&db).unwrap(), 0);
    assert_eq!(get_audio_current(&db).unwrap(), "");
}

#[test]
fn playlist_query_count_and_bulk_removal_are_scoped_to_file_ids() {
    let db = test_db("playlist_query");
    let tracks = [
        audio("/one", "First", 1),
        audio("/two", "Second", 2),
        audio("/three", "Third", 3),
    ];
    let first = create_playlist(&db, "First").unwrap();
    let second = create_playlist(&db, "Second").unwrap();
    indexed_add_playlist_items(&db, &first.id, &tracks).unwrap();
    add_playlist_items(&db, &second.id, &tracks).unwrap();
    assert_eq!(
        playlist_filtered_count(&db, &first.id, "text:Second").unwrap(),
        1
    );
    assert_eq!(
        playlist_items_page(&db, &first.id, 0, 10, "ids:test:/two").unwrap()[0].path,
        "/two"
    );
    assert_eq!(
        playlist_filtered_count(&db, &first.id, "NOT ids:test:/two").unwrap(),
        2
    );
    assert!(remove_playlist_query(&db, &first.id, "").is_err());
    assert_eq!(
        remove_playlist_query(&db, &first.id, "ids:test:/one,test:/two,test:/absent").unwrap(),
        2
    );
    assert_eq!(
        remove_playlist_query(&db, &first.id, "ids:test:/one,test:/two").unwrap(),
        0
    );
    assert_eq!(playlist_item_count(&db, &first.id).unwrap(), 1);
    assert_eq!(playlist_item_count(&db, &second.id).unwrap(), 3);
    assert_eq!(
        playlist_items_page(&db, &second.id, 0, 10, "")
            .unwrap()
            .iter()
            .map(|track| track.path.clone())
            .collect::<Vec<_>>(),
        tracks
            .iter()
            .map(|track| track.path.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn file_cleanup_clears_a_selected_uri_alias_even_when_it_is_not_queued() {
    let db = test_db("uri_cleanup");
    db.use_platform_audio_catalog().unwrap();
    let uri = "content://media/external/audio/media/42";
    db.cache_platform_audio(
        &[crate::db::audio_catalog::AudioMedia {
            file_id: Some("42".into()),
            aliases: vec![uri.into()],
            track: AudioTrack::from_path_stem("/music/one.mp3"),
        }],
        true,
    )
    .unwrap();
    save_audio_current(&db, uri).unwrap();
    remove_paths(&db, &["/music/one.mp3".into()]).unwrap();
    assert_eq!(get_audio_current(&db).unwrap(), "");
    save_audio_current(&db, uri).unwrap();
    db.audio_transaction(|tx, state| remove_root_paths_conn(tx, "/music", state))
        .unwrap();
    assert_eq!(get_audio_current(&db).unwrap(), "");
    assert_eq!(db.audio_current_file_id(), None);
}

#[test]
fn adding_to_an_idle_queue_selects_the_first_indexed_input_without_starting_playback() {
    let db = test_db("queue-initial-selection");
    let tracks = [audio("/b.mp3", "B", 1000), audio("/a.mp3", "A", 2000)];
    register_audio_tracks(&db, &tracks);
    enqueue_selecting_if_idle(&db, &tracks).unwrap();
    assert_eq!(get_audio_current(&db).unwrap(), "/b.mp3");
    assert_eq!(
        queue_page(&db, 0, 10, "")
            .unwrap()
            .iter()
            .map(|track| track.path.as_str())
            .collect::<Vec<_>>(),
        ["/b.mp3", "/a.mp3"]
    );
    assert!(
        crate::media::audio::overview::history(&db)
            .unwrap()
            .is_empty()
    );
    let before = crate::media::audio::playback::snapshot(&db).unwrap();
    enqueue_selecting_if_idle(&db, &tracks[1..]).unwrap();
    assert_eq!(
        crate::media::audio::playback::snapshot(&db).unwrap(),
        before
    );
}

#[test]
fn initial_queue_selection_is_unchanged_on_invalid_inputs_and_sql_failure() {
    let db = test_db("queue-selection-atomic");
    let tracks = [audio("/b.mp3", "B", 1000), audio("/a.mp3", "A", 2000)];
    register_audio_tracks(&db, &tracks);
    let before = crate::media::audio::playback::snapshot(&db).unwrap();
    let mut invalid = tracks[1].clone();
    invalid.duration_ms = -1;
    assert!(enqueue_selecting_if_idle(&db, &[tracks[0].clone(), invalid]).is_err());
    assert!(queue_page(&db, 0, 10, "").unwrap().is_empty());
    assert_eq!(
        crate::media::audio::playback::snapshot(&db).unwrap(),
        before
    );
    db.with_conn(|connection| connection.execute_batch("CREATE TRIGGER reject_second_queue_insert BEFORE INSERT ON audio_queue_items WHEN NEW.file_id='test:/a.mp3' BEGIN SELECT RAISE(ABORT,'owned queue failure'); END;")).unwrap();
    assert!(enqueue_selecting_if_idle(&db, &tracks).is_err());
    assert!(queue_page(&db, 0, 10, "").unwrap().is_empty());
    assert_eq!(
        crate::media::audio::playback::snapshot(&db).unwrap(),
        before
    );
}

#[test]
fn ordinary_enqueue_keeps_its_existing_selection_behavior() {
    let db = test_db("ordinary-queue-selection");
    let tracks = [audio("/one.mp3", "One", 1000)];
    register_audio_tracks(&db, &tracks);
    enqueue(&db, &tracks, false).unwrap();
    assert_eq!(get_audio_current(&db).unwrap(), "");
    enqueue_selecting_if_idle(&db, &[]).unwrap();
    assert_eq!(get_audio_current(&db).unwrap(), "");
    enqueue_selecting_if_idle(&db, &[audio("/unindexed.mp3", "Missing", 1000)]).unwrap();
    assert_eq!(get_audio_current(&db).unwrap(), "");
}

#[test]
fn replacing_queue_selects_a_track_and_keeps_old_state_when_a_track_is_missing() {
    let db = test_db("replace-queue");
    let old = audio("/queue/old.mp3", "Old", 1);
    indexed_enqueue(&db, &[old.clone()], false).unwrap();
    indexed_current(&db, &old.path).unwrap();
    let before = crate::media::audio::playback::snapshot(&db).unwrap();
    let incoming = [
        audio("/queue/one.mp3", "One", 1),
        audio("/queue/two.mp3", "Two", 2),
    ];
    register_audio_tracks(&db, &incoming);
    let mut broken = incoming.to_vec();
    broken.push(audio("/queue/missing.mp3", "Missing", 1));
    assert!(replace_queue(&db, &broken, false).is_err());
    let mut changed = incoming[0].clone();
    changed.file_id = Some("different-track".into());
    assert!(replace_queue(&db, &[changed], false).is_err());
    assert_eq!(
        paths_of(&queue_page(&db, 0, 10, "").unwrap()),
        vec![old.path]
    );
    assert_eq!(
        crate::media::audio::playback::snapshot(&db).unwrap(),
        before
    );
    let selected = replace_queue(&db, &incoming, false).unwrap().unwrap();
    assert_eq!(selected.path, incoming[0].path);
    assert_eq!(
        paths_of(&queue_page(&db, 0, 10, "").unwrap()),
        paths_of(&incoming)
    );
    assert_eq!(db.audio_current_file_id(), selected.file_id);
    let selected = replace_queue(&db, &incoming, true).unwrap().unwrap();
    assert!(incoming.iter().any(|track| track.path == selected.path));
    assert!(replace_queue(&db, &[], false).unwrap().is_none());
    assert_eq!(queue_total(&db).unwrap(), 0);
    assert!(db.audio_current_file_id().is_none());
}
