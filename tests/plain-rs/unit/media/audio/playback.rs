use super::*;
use crate::media::audio::queue as audio_queue;
#[path = "../../library/fixtures.rs"]
mod fixtures;
use fixtures::{indexed_current, register_audio};

#[test]
fn in_memory_databases_have_independent_playback_state() {
    let first = Db::open(std::path::Path::new(":memory:")).unwrap();
    let second = Db::open(std::path::Path::new(":memory:")).unwrap();
    indexed_current(&first, "one").unwrap();
    seek(&first, 123).unwrap();
    assert_eq!(snapshot(&first.clone()).unwrap().position_ms, 123);
    assert_eq!(snapshot(&second).unwrap(), Playback::default());
}

#[test]
fn recreated_runtime_rejects_reports_from_the_previous_engine_session() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let db = Db::open(&path).unwrap();
    indexed_current(&db, "one").unwrap();
    let previous = snapshot(&db).unwrap();
    drop(db);
    let db = Db::open(&path).unwrap();
    register_audio(&db, &audio_queue::AudioTrack::from_path_stem("one"));
    let current = invalidate(&db).unwrap();
    assert!(!report(&db, "one", previous.revision, 123).unwrap());
    assert!(report(&db, "one", current.revision, 456).unwrap());
}

#[test]
fn reopening_remembers_only_the_current_track_and_preserves_queue_and_history() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let db = Db::open(&path).unwrap();
    let track = audio_queue::AudioTrack::from_path_stem("one");
    register_audio(&db, &track);
    let playlist = audio_queue::create_playlist(&db, "Saved playlist").unwrap();
    audio_queue::add_playlist_items(&db, &playlist.id, std::slice::from_ref(&track)).unwrap();
    crate::media::audio::queue::enqueue(&db, std::slice::from_ref(&track), false).unwrap();
    let row = prepare_track(&db, &track).unwrap();
    loaded(&db, &track.path, row.revision).unwrap();
    assert!(started(&db, &track, row.revision).unwrap());
    seek(&db, 123_456).unwrap();
    let playing = invalidate(&db).unwrap();
    assert!(report(&db, "one", playing.revision, 234_567).unwrap());
    loaded(&db, "one", playing.revision).unwrap();
    let id = db.audio_current_file_id().unwrap();
    assert_eq!(
        fixtures::history_row(&db, &id).unwrap().position_ms,
        234_567
    );
    assert_eq!(db.audio_current_file_id().unwrap(), id);
    assert!(
        crate::prefs::Prefs::load(&crate::prefs::default_path(dir.path()))
            .unwrap()
            .get::<String>("audio_current_path")
            .unwrap()
            .is_none()
    );
    drop(db);

    let db = Db::open(&path).unwrap();
    register_audio(&db, &track);
    let restored = snapshot(&db).unwrap();
    assert_eq!(restored.path, "one");
    assert_eq!(restored.position_ms, 0);
    db.audio().read(|state| {
        assert_eq!(state.load_revision, -1);
        assert_eq!(state.started_revision, -1);
    });
    assert!(!started(&db, &track, playing.revision).unwrap());
    assert_eq!(audio_queue::queue_page(&db, 0, 10, "").unwrap().len(), 1);
    assert_eq!(
        crate::media::audio::overview::history(&db).unwrap()[0].play_count,
        1
    );
    assert_eq!(audio_queue::playlists(&db).unwrap()[0].0.id, playlist.id);
    assert_eq!(
        audio_queue::playlist_item_records_page(&db, &playlist.id, 0, 10, "").unwrap()[0]
            .audio_path,
        "one"
    );
    indexed_current(&db, "two").unwrap();
    drop(db);
    let reopened = Db::open(&path).unwrap();
    register_audio(&reopened, &audio_queue::AudioTrack::from_path_stem("two"));
    assert_eq!(snapshot(&reopened).unwrap().path, "two");
}

#[test]
fn failed_database_commit_restores_current_track_row_and_runtime() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("db")).unwrap();
    indexed_current(&db, "one").unwrap();
    let before = seek(&db, 123).unwrap();
    db.with_conn(|c| c.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE parent(id INTEGER PRIMARY KEY); CREATE TABLE child(parent_id INTEGER REFERENCES parent(id) DEFERRABLE INITIALLY DEFERRED);")).unwrap();
    assert!(
        db.audio_transaction(|tx, state| {
            state.current_file_id = Some("test:two".into());
            state.select("two")?;
            tx.execute("INSERT INTO child VALUES(1)", [])?;
            Ok(())
        })
        .is_err()
    );
    assert_eq!(snapshot(&db).unwrap(), before);
    let selected = db.audio_current_file_id().unwrap();
    assert_eq!(selected, "test:one");
}

#[test]
fn failed_current_media_write_keeps_the_source_and_runtime_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("db")).unwrap();
    indexed_current(&db, "one").unwrap();
    let before = seek(&db, 123).unwrap();
    register_audio(&db, &audio_queue::AudioTrack::from_path_stem("two"));
    let store = db.key_values();
    let stored = store.get("local_audio_current_file_id").unwrap();
    db.with_conn(|conn|conn.execute_batch("CREATE TRIGGER reject_current BEFORE INSERT ON key_values WHEN NEW.key='local_audio_current_file_id' BEGIN SELECT RAISE(ABORT,'current write failed'); END;")).unwrap();
    assert!(audio_queue::save_audio_current(&db, "two").is_err());
    db.with_conn(|conn| conn.execute_batch("DROP TRIGGER reject_current;"))
        .unwrap();
    assert_eq!(store.get("local_audio_current_file_id").unwrap(), stored);
    assert_eq!(snapshot(&db).unwrap(), before);
    assert_eq!(audio_queue::get_audio_current(&db).unwrap(), "one");
}
#[test]
fn stale_reports_cannot_overwrite_seek_or_returned_track() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let db = Db::open(&path).unwrap();
    indexed_current(&db, "one").unwrap();
    let first = snapshot(&db).unwrap();
    assert!(report(&db, "one", first.revision, 3_000_000_123).unwrap());
    assert_eq!(snapshot(&db.clone()).unwrap().position_ms, 3_000_000_123);
    let resumed = invalidate(&db).unwrap();
    assert_eq!(resumed.position_ms, 3_000_000_123);
    assert!(!report(&db, "one", first.revision, 0).unwrap());
    let sought = seek(&db, 0).unwrap();
    assert!(!report(&db, "one", resumed.revision, 100).unwrap());
    assert!(report(&db, "one", sought.revision, 456).unwrap());
    indexed_current(&db, "two").unwrap();
    assert_eq!(snapshot(&db).unwrap().position_ms, 0);
    indexed_current(&db, "one").unwrap();
    assert!(!report(&db, "one", sought.revision, 999).unwrap());
    assert_eq!(snapshot(&db).unwrap().position_ms, 0);
    assert!(seek(&db, -1).is_err());
    assert!(report(&db, "one", 0, -1).is_err());
}
#[test]
fn current_and_progress_reset_are_atomic() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("db")).unwrap();
    indexed_current(&db, "one").unwrap();
    audio_queue::save_audio_current(&db, "one").unwrap();
    seek(&db, 123).unwrap();
    register_audio(&db, &audio_queue::AudioTrack::from_path_stem("two"));
    let store = db.key_values();
    let stored = store.get("local_audio_current_file_id").unwrap();
    db.with_conn(|conn|conn.execute_batch("CREATE TRIGGER reject_current BEFORE INSERT ON key_values WHEN NEW.key='local_audio_current_file_id' BEGIN SELECT RAISE(ABORT,'current write failed'); END;")).unwrap();
    assert!(audio_queue::save_audio_current(&db, "two").is_err());
    db.with_conn(|conn| conn.execute_batch("DROP TRIGGER reject_current;"))
        .unwrap();
    assert_eq!(store.get("local_audio_current_file_id").unwrap(), stored);
    assert_eq!(audio_queue::get_audio_current(&db).unwrap(), "one");
    assert_eq!(snapshot(&db).unwrap().position_ms, 123);
    assert_eq!(seek(&db, 456).unwrap().position_ms, 456);
}

#[test]
fn playback_start_is_once_per_revision_and_failed_history_write_can_retry() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("db")).unwrap();
    let track = audio_queue::AudioTrack::from_path_stem("one");
    register_audio(&db, &track);
    let row = prepare_track(&db, &track).unwrap();
    loaded(&db, &track.path, row.revision).unwrap();
    assert!(started(&db, &track, row.revision).unwrap());
    assert!(!started(&db, &track, row.revision).unwrap());
    assert_eq!(
        crate::media::audio::overview::history(&db).unwrap()[0].play_count,
        1
    );
    let newer = prepare_track(&db, &track).unwrap();
    loaded(&db, &track.path, newer.revision).unwrap();
    assert!(!started(&db, &track, row.revision).unwrap());
    db.with_conn(|c| c.execute_batch("CREATE TRIGGER fail_history BEFORE INSERT ON media_play_history BEGIN SELECT RAISE(ABORT,'fail'); END")).unwrap();
    assert!(started(&db, &track, newer.revision).is_err());
    db.with_conn(|c| c.execute_batch("DROP TRIGGER fail_history"))
        .unwrap();
    assert!(started(&db, &track, newer.revision).unwrap());
    assert_eq!(
        crate::media::audio::overview::history(&db).unwrap()[0].play_count,
        2
    );
}

#[test]
fn buffered_start_survives_pause_resume_and_seek_but_not_track_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("db")).unwrap();
    let track = audio_queue::AudioTrack::from_path_stem("one");
    register_audio(&db, &track);
    let first = prepare_track(&db, &track).unwrap();
    loaded(&db, &track.path, first.revision).unwrap();
    invalidate(&db).unwrap();
    seek(&db, 100).unwrap();
    assert!(started(&db, &track, first.revision).unwrap());
    let next = prepare_track(&db, &track).unwrap();
    loaded(&db, &track.path, next.revision).unwrap();
    assert!(!started(&db, &track, first.revision).unwrap());
    assert!(started(&db, &track, next.revision).unwrap());
    assert_eq!(
        crate::media::audio::overview::history(&db).unwrap()[0].play_count,
        2
    );
    indexed_current(&db, "other").unwrap();
    indexed_current(&db, "one").unwrap();
    assert!(!started(&db, &track, next.revision).unwrap());
}

#[test]
fn progress_reports_do_not_count_plays_and_only_loaded_revisions_can_start() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let track = audio_queue::AudioTrack::from_path_stem("one");
    register_audio(&db, &track);
    let selected = prepare_track(&db, &track).unwrap();
    assert!(report(&db, &track.path, selected.revision, 123).unwrap());
    assert!(
        crate::media::audio::overview::history(&db)
            .unwrap()
            .is_empty()
    );
    assert!(!started(&db, &track, selected.revision).unwrap());
    loaded(&db, &track.path, selected.revision).unwrap();
    assert!(started(&db, &track, selected.revision).unwrap());
    assert!(!started(&db, &track, selected.revision).unwrap());
    assert!(report(&db, &track.path, selected.revision, 456).unwrap());
    let row = fixtures::history_row(&db, "test:one").unwrap();
    assert_eq!(row.play_count, 1);
    assert_eq!(row.position_ms, 456);
    let replay = prepare_track(&db, &track).unwrap();
    assert!(!report(&db, &track.path, selected.revision, 999).unwrap());
    loaded(&db, &track.path, replay.revision).unwrap();
    assert!(started(&db, &track, replay.revision).unwrap());
    assert_eq!(
        fixtures::history_row(&db, "test:one").unwrap().play_count,
        2
    );
}

#[test]
fn new_selection_rejects_buffered_native_start_even_for_the_same_path() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let track = audio_queue::AudioTrack::from_path_stem("one");
    register_audio(&db, &track);
    let first = prepare_track(&db, &track).unwrap();
    loaded(&db, &track.path, first.revision).unwrap();
    let replacement = prepare_track(&db, &track).unwrap();
    assert!(!started(&db, &track, first.revision).unwrap());
    assert!(!started(&db, &track, replacement.revision).unwrap());
    loaded(&db, &track.path, replacement.revision).unwrap();
    assert!(started(&db, &track, replacement.revision).unwrap());
    assert!(!started(&db, &track, first.revision).unwrap());
    assert_eq!(
        fixtures::history_row(&db, "test:one").unwrap().play_count,
        1
    );
}

#[test]
fn failed_start_preserves_position_and_can_retry_the_same_loaded_revision() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let track = audio_queue::AudioTrack::from_path_stem("one");
    register_audio(&db, &track);
    let selected = prepare_track(&db, &track).unwrap();
    loaded(&db, &track.path, selected.revision).unwrap();
    report(&db, &track.path, selected.revision, 123).unwrap();
    let before = snapshot(&db).unwrap();
    db.with_conn(|c| c.execute_batch("CREATE TRIGGER fail_report BEFORE INSERT ON media_play_history BEGIN SELECT RAISE(ABORT,'fail'); END")).unwrap();
    assert!(started(&db, &track, selected.revision).is_err());
    assert_eq!(snapshot(&db).unwrap(), before);
    assert!(fixtures::history_row(&db, "test:one").is_none());
    db.audio()
        .read(|state| assert_eq!(state.started_revision, -1));
    db.with_conn(|c| c.execute_batch("DROP TRIGGER fail_report"))
        .unwrap();
    assert!(started(&db, &track, selected.revision).unwrap());
    let row = fixtures::history_row(&db, "test:one").unwrap();
    assert_eq!((row.play_count, row.position_ms), (1, 123));
}

#[test]
fn unindexed_playback_does_not_create_history_and_invalid_reports_have_no_effect() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let track = audio_queue::AudioTrack::from_path_stem("unindexed");
    let selected = prepare_track(&db, &track).unwrap();
    loaded(&db, &track.path, selected.revision).unwrap();
    assert!(started(&db, &track, selected.revision).unwrap());
    assert!(
        crate::media::audio::overview::history(&db)
            .unwrap()
            .is_empty()
    );
    for invalid in [
        Playback {
            path: String::new(),
            ..selected.clone()
        },
        Playback {
            revision: -1,
            ..selected.clone()
        },
        Playback {
            position_ms: -1,
            ..selected.clone()
        },
    ] {
        assert!(report(&db, &invalid.path, invalid.revision, invalid.position_ms).is_err());
    }
    assert_eq!(snapshot(&db).unwrap(), selected);
}
