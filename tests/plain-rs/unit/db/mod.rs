//! Unit tests for `src/library/db/mod.rs` (Db open + DDL).

#[path = "../library/fixtures.rs"]
mod fixtures;
use crate::db::Db;
use fixtures::test_db;

/// Fresh temp dir per call (label + pid + nanos): isolates parallel
/// tests and leaves nothing behind from a previous run.
pub fn unique_tmp_dir(label: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "plain-rs-db-{label}-{}-{nanos}",
        std::process::id()
    ))
}

#[test]
fn open_creates_every_table_once() {
    let db = test_db("open_tables");
    assert_eq!(
        db.with_conn(
            |conn| conn.pragma_query_value(None, "user_version", |row| row.get::<_, i32>(0))
        )
        .unwrap(),
        1
    );
    let tables: Vec<String> = db.with_conn(|conn| {
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap();
        stmt.query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
    });
    for expected in [
        "audio_queue_items",
        "audio_playlists",
        "audio_playlist_items",
        "media_play_history",
        "tags",
        "tag_relations",
        "favorite_folders",
    ] {
        assert!(
            tables.iter().any(|t| t == expected),
            "missing table {expected}"
        );
    }
    assert!(!tables.iter().any(|t| t == "settings"));
    assert!(!tables.iter().any(|t| t == "audio_playback"));
    for removed in [
        "audio_queue_source",
        "audio_play_history",
        "video_play_progress",
    ] {
        assert!(!tables.iter().any(|table| table == removed));
    }
    for removed in [
        "file_tasks",
        "file_deletions",
        "file_task_effects",
        "file_task_audio_effects",
    ] {
        assert!(!tables.iter().any(|table| table == removed));
    }
    // Reopening the same file is fine (CREATE IF NOT EXISTS). The path
    // must be unique: this file is mounted as `db::tests`,
    // `db::chat_tests` and `db::library_tests` in one binary, and all
    // three run concurrently.
    let dir = tempfile::Builder::new()
        .prefix("plain-rs-library-reopen2-")
        .tempdir()
        .unwrap();
    let path = dir.path().join("reopen.db");
    {
        let _a = Db::open(&path).unwrap();
    }
    let _b = Db::open(&path).unwrap();
}

#[test]
fn current_track_defaults_to_none() {
    let db = test_db("current_default");
    assert_eq!(db.audio_current_file_id(), None);
    assert_eq!(
        crate::media::audio::queue::get_audio_current(&db).unwrap(),
        ""
    );
}
