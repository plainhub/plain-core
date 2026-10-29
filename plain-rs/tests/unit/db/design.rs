use crate::db::{Db, MediaFileMetadata, TrashedMessageRow};
use crate::enums::DataType;
use std::path::Path;
use std::sync::Arc;

fn register(db: &Db) -> String {
    db.record_media_files(&[MediaFileMetadata {
        album: String::new(),
        album_artist: String::new(),

        data_type: DataType::Audio,
        path: "/track.mp3".into(),
        title: "Track".into(),
        artist: String::new(),
        bucket_id: "/".into(),
        size_bytes: 1,
        duration_ms: 1,
        cover_image_id: None,
        width: 0,
        height: 0,
        rotation: 0,
        taken_at: None,
        created_at: "2026-01-01T08:00:00+08:00".into(),
        updated_at: "2026-01-01T08:00:00+08:00".into(),
    }])
    .unwrap()
    .remove(0)
    .id
}

#[test]
fn installed_schema_matches_generated_definitions() {
    fn definitions(conn: &rusqlite::Connection) -> Vec<(String, String, String)> {
        conn.prepare(
            "SELECT type,name,sql FROM sqlite_master WHERE sql IS NOT NULL ORDER BY type,name",
        )
        .unwrap()
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get::<_, String>(2)?
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
            ))
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
    }
    let expected = rusqlite::Connection::open_in_memory().unwrap();
    expected
        .execute_batch(include_str!("../../../../schema/database.sql"))
        .unwrap();
    let db = Db::open(Path::new(":memory:")).unwrap();
    assert_eq!(
        db.with_conn(|conn| definitions(conn)),
        definitions(&expected)
    );
}

#[test]
fn current_track_reopens_from_kv_without_playback_progress() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("db");
    let db = Db::open(&path).unwrap();
    let id = register(&db);
    crate::media::audio::queue::save_audio_current(&db, "/track.mp3").unwrap();
    crate::media::audio::playback::seek(&db, 123).unwrap();
    let stored = db
        .key_values()
        .get("local_audio_current_file_id")
        .unwrap()
        .unwrap();
    assert_eq!(stored, id);
    drop(db);
    let reopened = Db::open(&path).unwrap();
    assert_eq!(
        crate::media::audio::playback::snapshot(&reopened)
            .unwrap()
            .position_ms,
        0
    );
    assert_eq!(reopened.audio_current_file_id(), Some(id));
}

#[test]
fn sms_and_mms_with_equal_ids_restore_independently() {
    let db = Db::open(Path::new(":memory:")).unwrap();
    let rows = [false, true].map(|is_mms| TrashedMessageRow {
        message_id: "42".into(),
        is_mms,
        trashed_at: "2026-01-01T00:00:00.000Z".into(),
    });
    db.trashed_message_save_many(&rows).unwrap();
    let mut ids = db.trashed_message_ids().unwrap();
    ids.sort();
    assert_eq!(ids, ["42", "mms_42"]);
    assert_eq!(db.trashed_message_delete_by_ids(&["42".into()]).unwrap(), 1);
    assert_eq!(db.trashed_message_ids().unwrap(), ["mms_42"]);
    assert_eq!(
        db.trashed_message_delete_by_ids(&["mms_42".into()])
            .unwrap(),
        1
    );
}

#[test]
fn media_storage_uses_nullable_cover_and_canonical_utc_times() {
    let db = Db::open(Path::new(":memory:")).unwrap();
    let id = register(&db);
    let item = db
        .media_file_by_path(DataType::Audio, "/track.mp3")
        .unwrap()
        .unwrap();
    assert_eq!(item.cover_image_id, None);
    assert_eq!(item.created_at, "2026-01-01T00:00:00.000Z");
    assert_eq!(item.updated_at, item.created_at);
    db.with_conn(|conn| {
        for assignment in [
            "id=''",
            "favorited=2",
            "missing=-1",
            "rotation=45",
            "size_bytes=-1",
            "duration_ms=-1",
        ] {
            assert!(
                conn.execute(&format!("UPDATE files SET {assignment} WHERE id=?1"), [&id])
                    .is_err(),
                "{assignment}"
            );
        }
    });
}

#[test]
fn embedding_storage_rejects_invalid_vectors() {
    let db = Db::open(Path::new(":memory:")).unwrap();
    for bytes in [
        Vec::new(),
        vec![0, 1, 2],
        f32::NAN.to_be_bytes().to_vec(),
        f32::INFINITY.to_be_bytes().to_vec(),
    ] {
        assert!(
            db.embedding_save(&crate::db::ImageEmbeddingRow {
                file_id: "image".into(),
                path: "/image.jpg".into(),
                embedding_base64: crate::utils::base64::base64_encode(&bytes),
                created_at: "2026-01-01T00:00:00.000Z".into(),
                updated_at: "2026-01-01T00:00:00.000Z".into(),
            })
            .is_err()
        );
    }
    assert_eq!(db.embedding_count().unwrap(), 0);
}

#[test]
fn feed_entry_identity_is_scoped_to_the_source_not_the_url() {
    let db = Db::open(Path::new(":memory:")).unwrap();
    let now = "2026-01-01T00:00:00.000Z";
    db.feed_save("feed", "Feed", "https://example.test/feed", false, now)
        .unwrap();
    let entry = |id: &str, source_id: &str| crate::db::notes_feeds::FeedEntryRow {
        id: id.into(),
        feed_id: "feed".into(),
        title: "Entry".into(),
        url: "https://example.test/article".into(),
        image: String::new(),
        description: String::new(),
        author: String::new(),
        content: String::new(),
        source_entry_id: source_id.into(),
        published_at: now.into(),
        is_read: false,
        created_at: now.into(),
        updated_at: now.into(),
    };
    let count = db
        .feed_entries_insert(&[
            entry("one", "source-one"),
            entry("two", "source-two"),
            entry("duplicate", "source-one"),
        ])
        .unwrap();
    assert_eq!(count.len(), 2);
    assert_eq!(db.feed_entry_count("").unwrap(), 2);
}
