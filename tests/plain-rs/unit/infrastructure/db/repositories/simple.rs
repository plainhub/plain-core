#[path = "../../../features/library/fixtures.rs"]
mod fixtures;
use fixtures::*;

use crate::infrastructure::db::models::simple::ArchivedConversationRow;
use crate::infrastructure::db::models::simple::ClipboardRow;
use crate::infrastructure::db::models::simple::ImageEmbeddingRow;
use crate::infrastructure::db::models::simple::PomodoroItemRow;
use crate::infrastructure::db::models::simple::SessionRow;
use crate::infrastructure::db::models::simple::ShareRow;
use crate::infrastructure::db::models::simple::TrashedMessageRow;

fn clipboard(id: &str, created_at: &str) -> ClipboardRow {
    ClipboardRow {
        id: id.into(),
        text: format!("text-{id}"),
        hash: format!("hash-{id}"),
        source: String::new(),
        label: "label".into(),
        sensitive: id == "sensitive",
        created_at: created_at.into(),
    }
}

#[test]
fn clipboard_round_trips_and_orders_by_created_at() {
    let db = test_db("clipboard");
    db.clipboard_save(&clipboard("a", "2026-01-01T00:00:00.000Z"))
        .unwrap();
    db.clipboard_save(&clipboard("b", "2026-02-01T00:00:00.000Z"))
        .unwrap();

    let got = db.clipboard_get("a").unwrap().expect("row a");
    assert_eq!(got.text, "text-a");
    assert!(!got.sensitive);

    let latest = db.clipboard_latest().unwrap().expect("latest");
    assert_eq!(latest.id, "b");

    let by_hash = db
        .clipboard_latest_by_hash("hash-a")
        .unwrap()
        .expect("by hash");
    assert_eq!(by_hash.id, "a");

    // Saving the same id replaces rather than duplicating.
    let mut updated = clipboard("a", "2026-01-01T00:00:00.000Z");
    updated.text = "edited".into();
    db.clipboard_save(&updated).unwrap();
    assert_eq!(db.clipboard_get("a").unwrap().unwrap().text, "edited");
    assert_eq!(db.clipboard_count_rows().unwrap(), 2);
}

#[test]
fn clipboard_sensitive_flag_is_stored_as_a_boolean() {
    let db = test_db("clipboard_sensitive");
    db.clipboard_save(&clipboard("sensitive", "2026-01-01T00:00:00.000Z"))
        .unwrap();
    assert!(db.clipboard_get("sensitive").unwrap().unwrap().sensitive);
}

#[test]
fn clipboard_delete_by_ids_and_clear() {
    let db = test_db("clipboard_delete");
    for id in ["a", "b", "c"] {
        db.clipboard_save(&clipboard(id, "2026-01-01T00:00:00.000Z"))
            .unwrap();
    }
    assert_eq!(
        db.clipboard_delete_by_ids(&["a".into(), "c".into()])
            .unwrap(),
        2
    );
    assert_eq!(db.clipboard_count_rows().unwrap(), 1);
    // An empty id list must delete nothing rather than matching all rows.
    assert_eq!(db.clipboard_delete_by_ids(&[]).unwrap(), 0);
    assert_eq!(db.clipboard_clear().unwrap(), 1);
    assert_eq!(db.clipboard_count_rows().unwrap(), 0);
}

#[test]
fn session_save_get_and_touch() {
    let db = test_db("session");
    let row = SessionRow {
        url_token: "resource-key".into(),
        client_id: "client-1".into(),
        name: "Browser".into(),
        r#type: "WEB".into(),
        client_ip: "192.168.1.5".into(),
        os_name: "macOS".into(),
        os_version: "15.0".into(),
        browser_name: "Chrome".into(),
        browser_version: "120".into(),
        token: "tok".into(),
        last_seen_at: Some("2026-01-01T00:00:00.000Z".into()),
        created_at: "2026-01-01T00:00:00.000Z".into(),
        updated_at: "2026-01-01T00:00:00.000Z".into(),
    };
    db.session_save(&row).unwrap();

    let got = db.session_get("client-1").unwrap().expect("session");
    assert_eq!(got.r#type, "WEB");
    assert_eq!(got.browser_name, "Chrome");
    assert_eq!(got.url_token, row.url_token);

    db.session_touch(&[("client-1".into(), "2026-03-01T00:00:00.000Z".into())])
        .unwrap();
    let touched = db.session_get("client-1").unwrap().unwrap();
    assert_eq!(
        touched.last_seen_at.as_deref(),
        Some("2026-03-01T00:00:00.000Z")
    );
    // Touching must not disturb the rest of the row.
    assert_eq!(touched.browser_version, "120");

    assert_eq!(db.session_list().unwrap().len(), 1);
    assert_eq!(db.session_delete("client-1").unwrap(), 1);
    assert!(db.session_get("client-1").unwrap().is_none());
}

#[test]
fn session_list_orders_by_last_active_desc() {
    let db = test_db("session_order");
    for (id, at) in [
        ("old", "2026-01-01T00:00:00.000Z"),
        ("new", "2026-05-01T00:00:00.000Z"),
        ("mid", "2026-03-01T00:00:00.000Z"),
    ] {
        db.session_save(&SessionRow {
            url_token: "resource-key".into(),
            client_id: id.into(),
            name: id.into(),
            r#type: "WEB".into(),
            client_ip: "10.0.0.1".into(),
            os_name: String::new(),
            os_version: String::new(),
            browser_name: String::new(),
            browser_version: String::new(),
            token: String::new(),
            last_seen_at: Some(at.into()),
            created_at: at.into(),
            updated_at: at.into(),
        })
        .unwrap();
    }
    let ids: Vec<String> = db
        .session_list()
        .unwrap()
        .into_iter()
        .map(|r| r.client_id)
        .collect();
    assert_eq!(ids, vec!["new", "mid", "old"]);
}

#[test]
fn share_round_trip() {
    let db = test_db("share");
    let row = ShareRow {
        id: "share-1".into(),
        name: "Photos".into(),
        password: String::new(),
        url_token: "url-token".into(),
        expires_at: Some("2027-01-01T00:00:00.000Z".into()),
        read_only: true,
        data: "[\"/a\"]".into(),
        created_at: "2026-01-01T00:00:00.000Z".into(),
        updated_at: "2026-01-01T00:00:00.000Z".into(),
    };
    assert!(db.share_insert(&row).unwrap());
    let got = db.share_get("share-1").unwrap().expect("share");
    assert!(got.read_only);
    assert_eq!(got.url_token, "url-token");
    assert_eq!(got.expires_at.as_deref(), Some("2027-01-01T00:00:00.000Z"));
    assert_eq!(db.share_list().unwrap().len(), 1);
    let mut collision = row.clone();
    collision.name = "Replacement".into();
    collision.url_token = "replacement-token".into();
    assert!(!db.share_insert(&collision).unwrap());
    let preserved = db.share_get(&row.id).unwrap().unwrap();
    assert_eq!(preserved.name, row.name);
    assert_eq!(preserved.url_token, row.url_token);
    assert_eq!(db.share_list().unwrap().len(), 1);
    assert_eq!(db.share_delete("share-1").unwrap(), 1);
}

#[test]
fn pomodoro_queries() {
    let db = test_db("pomodoro");
    for (id, date, completed) in [
        ("p1", "2026-01-01", 3),
        ("p2", "2026-02-01", 4),
        ("p3", "2026-03-01", 5),
    ] {
        db.pomodoro_save(&PomodoroItemRow {
            id: id.into(),
            date: date.into(),
            completed_count: completed,
            total_work_secs: 60,
            total_break_secs: 30,
            created_at: "2026-01-01T00:00:00.000Z".into(),
            updated_at: "2026-01-01T00:00:00.000Z".into(),
        })
        .unwrap();
    }
    let dates: Vec<String> = db
        .pomodoro_list()
        .unwrap()
        .into_iter()
        .map(|r| r.date)
        .collect();
    assert_eq!(dates, vec!["2026-03-01", "2026-02-01", "2026-01-01"]);

    let by_date = db.pomodoro_get_by_date("2026-02-01").unwrap().expect("row");
    assert_eq!(by_date.completed_count, 4);

    let recent = db.pomodoro_recent("2026-02-01", 10).unwrap();
    assert_eq!(recent.len(), 2);

    assert_eq!(db.pomodoro_total_completed().unwrap(), 12);
    assert_eq!(db.pomodoro_delete("p1").unwrap(), 1);
}

#[test]
fn image_embedding_blob_survives_the_base64_boundary() {
    let db = test_db("embedding");
    // 0..=255 exercises every byte value through the JSON/base64 hop.
    let raw: Vec<u8> = (0u8..=255).collect();
    let encoded = plain_libs::utils::base64::base64_encode(&raw);
    db.embedding_save(&ImageEmbeddingRow {
        file_id: "e1".into(),
        path: "/photos/a.jpg".into(),
        embedding_base64: encoded.clone(),
        created_at: "2026-01-01T00:00:00.000Z".into(),
        updated_at: "2026-01-01T00:00:00.000Z".into(),
    })
    .unwrap();

    let rows = db.embedding_list().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].path, "/photos/a.jpg");
    assert_eq!(rows[0].embedding_base64, encoded);
    assert_eq!(db.embedding_ids().unwrap(), vec!["e1".to_string()]);
    assert_eq!(db.embedding_count().unwrap(), 1);

    assert_eq!(db.embedding_delete_by_ids(&["e1".into()]).unwrap(), 1);
    assert_eq!(db.embedding_count().unwrap(), 0);
}

#[test]
fn image_embedding_delete_all() {
    let db = test_db("embedding_all");
    for id in ["a", "b"] {
        db.embedding_save(&ImageEmbeddingRow {
            file_id: id.into(),
            path: format!("/{id}.jpg"),
            embedding_base64: "P4AAAA==".into(),
            created_at: "2026-01-01T00:00:00.000Z".into(),
            updated_at: "2026-01-01T00:00:00.000Z".into(),
        })
        .unwrap();
    }
    assert_eq!(db.embedding_delete_all().unwrap(), 2);
    assert_eq!(db.embedding_count().unwrap(), 0);
}

#[test]
fn archived_conversation_round_trip() {
    let db = test_db("archived");
    db.archived_conversation_save(&ArchivedConversationRow {
        conversation_id: "conv-1".into(),
        archived_at: "2026-01-01T00:00:00.000Z".into(),
    })
    .unwrap();
    let rows = db.archived_conversation_list().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].conversation_id, "conv-1");
    assert_eq!(db.archived_conversation_delete("conv-1").unwrap(), 1);
    assert!(db.archived_conversation_list().unwrap().is_empty());
}

#[test]
fn trashed_messages_bulk_insert_and_retention_cleanup() {
    let db = test_db("trashed");
    db.trashed_message_save_many(&[
        TrashedMessageRow {
            message_id: "1".into(),
            is_mms: false,
            trashed_at: "2026-01-01T00:00:00.000Z".into(),
        },
        TrashedMessageRow {
            message_id: "2".into(),
            is_mms: true,
            trashed_at: "2026-06-01T00:00:00.000Z".into(),
        },
    ])
    .unwrap();

    let mut ids = db.trashed_message_ids().unwrap();
    ids.sort();
    assert_eq!(ids, vec!["1".to_string(), "mms_2".to_string()]);

    assert_eq!(
        db.trashed_message_delete_older_than("2026-03-01T00:00:00.000Z")
            .unwrap(),
        1
    );
    assert_eq!(db.trashed_message_ids().unwrap(), vec!["mms_2".to_string()]);
    assert_eq!(
        db.trashed_message_delete_by_ids(&["mms_2".into()]).unwrap(),
        1
    );
    assert!(db.trashed_message_ids().unwrap().is_empty());
}

/// The FFI boundary needs both directions: Kotlin serializes a row on write
/// and deserializes the same shape on read.
#[test]
fn rows_round_trip_through_serde() {
    let original = ClipboardRow {
        id: "c1".into(),
        text: "hello".into(),
        hash: "h1".into(),
        source: "peer".into(),
        label: "label".into(),
        sensitive: true,
        created_at: "2026-01-01T00:00:00.000Z".into(),
    };
    let json = serde_json::to_string(&original).unwrap();
    let back: ClipboardRow = serde_json::from_str(&json).unwrap();
    assert_eq!(back.id, original.id);
    assert_eq!(back.text, original.text);
    assert_eq!(back.hash, original.hash);
    assert_eq!(back.source, original.source);
    assert_eq!(back.label, original.label);
    assert_eq!(back.sensitive, original.sensitive);
    assert_eq!(back.created_at, original.created_at);
}

/// Column names stay snake_case so the Kotlin data classes can map them
/// without a translation layer.
#[test]
fn serialized_field_names_are_snake_case() {
    let row = SessionRow {
        url_token: "resource-key".into(),
        client_id: "c".into(),
        name: "n".into(),
        r#type: "WEB".into(),
        client_ip: "1.2.3.4".into(),
        os_name: String::new(),
        os_version: String::new(),
        browser_name: String::new(),
        browser_version: String::new(),
        token: "t".into(),
        last_seen_at: None,
        created_at: "2026-01-01T00:00:00.000Z".into(),
        updated_at: "2026-01-01T00:00:00.000Z".into(),
    };
    let value: serde_json::Value = serde_json::to_value(&row).unwrap();
    let object = value.as_object().unwrap();
    for key in [
        "client_id",
        "client_ip",
        "os_name",
        "os_version",
        "browser_name",
        "browser_version",
        "last_seen_at",
        "created_at",
        "updated_at",
    ] {
        assert!(object.contains_key(key), "missing snake_case key {key}");
    }
    // The Rust keyword field is exposed under its plain name, not `r#type`.
    assert_eq!(object["type"], "WEB");
}

#[cfg(feature = "chat")]
#[test]
fn repeated_session_lookups_do_not_execute_sql() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let db = test_db("session-cache");
    let row = session_cache_row();
    db.session_save(&row).unwrap();
    let steps = Arc::new(AtomicUsize::new(0));
    let observed = steps.clone();
    db.with_conn(|connection| {
        connection.progress_handler(
            1,
            Some(move || {
                observed.fetch_add(1, Ordering::Relaxed);
                false
            }),
        )
    });
    for _ in 0..300 {
        assert_eq!(
            db.session_get(&row.client_id).unwrap().unwrap().token,
            row.token
        );
        assert!(db.session_get("unknown-session").unwrap().is_none());
    }
    db.with_conn(|connection| connection.progress_handler(0, None::<fn() -> bool>));
    assert_eq!(
        steps.load(Ordering::Relaxed),
        0,
        "cached session reads must not execute SQL"
    );
}

#[cfg(feature = "chat")]
fn session_cache_row() -> SessionRow {
    SessionRow {
        client_id: "cache-client".into(),
        name: "Browser".into(),
        r#type: "WEB".into(),
        client_ip: "127.0.0.1".into(),
        os_name: String::new(),
        os_version: String::new(),
        browser_name: String::new(),
        browser_version: String::new(),
        token: "session-key".into(),
        url_token: "resource-key".into(),
        last_seen_at: None,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    }
}

#[cfg(feature = "chat")]
#[test]
fn session_cache_tracks_mutations_clones_raw_sql_and_rollbacks() {
    let db = test_db("session-cache-mutations");
    let mut row = session_cache_row();
    db.session_save(&row).unwrap();
    let clone = db.clone();
    row.token = "rotated-session-key".into();
    row.url_token = "rotated-resource-key".into();
    db.session_save(&row).unwrap();
    assert_eq!(
        clone.session_get(&row.client_id).unwrap().unwrap().token,
        row.token
    );
    clone
        .session_rename(&row.client_id, "Renamed", "2026-02-01T00:00:00Z")
        .unwrap();
    assert_eq!(
        db.session_get(&row.client_id).unwrap().unwrap().name,
        "Renamed"
    );
    db.session_touch(&[(row.client_id.clone(), "2026-03-01T00:00:00+01:00".into())])
        .unwrap();
    assert_eq!(
        clone
            .session_get(&row.client_id)
            .unwrap()
            .unwrap()
            .last_seen_at
            .as_deref(),
        Some("2026-02-28T23:00:00.000Z")
    );
    db.with_conn(|connection| {
        connection.execute(
            "UPDATE sessions SET url_token='raw-resource-key' WHERE client_id=?1",
            [&row.client_id],
        )
    })
    .unwrap();
    assert_eq!(
        clone
            .session_get(&row.client_id)
            .unwrap()
            .unwrap()
            .url_token,
        "raw-resource-key"
    );
    db.with_conn(|connection| {
        let transaction = connection.unchecked_transaction().unwrap();
        transaction
            .execute(
                "UPDATE sessions SET token='uncommitted-key' WHERE client_id=?1",
                [&row.client_id],
            )
            .unwrap();
        transaction.rollback().unwrap();
    });
    assert_eq!(
        clone.session_get(&row.client_id).unwrap().unwrap().token,
        row.token
    );
    let mut invalid = row.clone();
    invalid.r#type = "invalid-session-type".into();
    assert!(db.session_save(&invalid).is_err());
    assert_eq!(
        db.session_get(&row.client_id).unwrap().unwrap().r#type,
        "WEB"
    );
    db.session_delete(&row.client_id).unwrap();
    assert!(clone.session_get(&row.client_id).unwrap().is_none());
    db.session_save(&row).unwrap();
    db.with_conn(|connection| connection.execute("DELETE FROM sessions", []))
        .unwrap();
    assert!(clone.session_get(&row.client_id).unwrap().is_none());
    for _ in 0..2 {
        db.session_save(&row).unwrap();
        db.with_conn(|connection| {
            connection
                .prepare_cached("DELETE FROM sessions")
                .unwrap()
                .execute([])
        })
        .unwrap();
        assert!(clone.session_get(&row.client_id).unwrap().is_none());
    }
}

#[cfg(feature = "chat")]
#[test]
fn session_cache_loads_persisted_rows_and_keeps_unchanged_lookups_in_memory() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("plain.db");
    let row = session_cache_row();
    let db = crate::infrastructure::db::Db::open(&path).unwrap();
    db.session_save(&row).unwrap();
    drop(db);
    let db = crate::infrastructure::db::Db::open(&path).unwrap();
    let steps = Arc::new(AtomicUsize::new(0));
    let observed = steps.clone();
    db.with_conn(|connection| {
        connection.progress_handler(
            1,
            Some(move || {
                observed.fetch_add(1, Ordering::Relaxed);
                false
            }),
        )
    });
    assert_eq!(
        db.session_get(&row.client_id).unwrap().unwrap().url_token,
        row.url_token
    );
    db.with_conn(|connection| connection.progress_handler(0, None::<fn() -> bool>));
    assert_eq!(steps.load(Ordering::Relaxed), 0);
}

#[test]
fn session_activity_is_visible_from_memory_and_300_touches_coalesce_into_one_write() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let db = test_db("buffered-session-activity");
    let row = session_cache_row();
    db.session_save(&row).unwrap();
    db.with_conn(|c|c.execute_batch("CREATE TEMP TABLE activity_writes(value INTEGER); CREATE TEMP TRIGGER activity_write AFTER UPDATE OF last_seen_at ON main.sessions BEGIN INSERT INTO activity_writes VALUES(1); END;")).unwrap();
    let steps = Arc::new(AtomicUsize::new(0));
    let observed = steps.clone();
    db.with_conn(|c| {
        c.progress_handler(
            1,
            Some(move || {
                observed.fetch_add(1, Ordering::Relaxed);
                false
            }),
        )
    });
    for _ in 0..300 {
        assert_eq!(
            db.session_touch(&[(row.client_id.clone(), "2026-03-01T00:00:00.000Z".into())])
                .unwrap(),
            1
        );
        assert_eq!(
            db.session_list().unwrap()[0].last_seen_at.as_deref(),
            Some("2026-03-01T00:00:00.000Z")
        );
    }
    db.with_conn(|c| c.progress_handler(0, None::<fn() -> bool>));
    assert_eq!(steps.load(Ordering::Relaxed), 0);
    assert_eq!(
        db.with_conn(
            |c| c.query_row("SELECT count(*) FROM activity_writes", [], |r| r
                .get::<_, i64>(0))
        )
        .unwrap(),
        0
    );
    assert!(
        db.flush_session_activity(std::time::Instant::now(), false)
            .unwrap()
    );
    assert_eq!(
        db.with_conn(
            |c| c.query_row("SELECT count(*) FROM activity_writes", [], |r| r
                .get::<_, i64>(0))
        )
        .unwrap(),
        1
    );
    assert_eq!(
        db.with_conn(|c| c.query_row(
            "SELECT last_seen_at FROM sessions WHERE client_id=?1",
            [&row.client_id],
            |r| r.get::<_, String>(0)
        ))
        .unwrap(),
        "2026-03-01T00:00:00.000Z"
    );
    assert!(
        !db.flush_session_activity(std::time::Instant::now(), true)
            .unwrap()
    );
}

#[test]
fn failed_session_activity_flush_retains_pending_values_and_revocation_discards_them() {
    let db = test_db("session-activity-failure");
    let mut row = session_cache_row();
    db.session_save(&row).unwrap();
    db.session_touch(&[(row.client_id.clone(), "2026-03-01T00:00:00.000Z".into())])
        .unwrap();
    db.with_conn(|c|c.execute_batch("CREATE TEMP TRIGGER reject_activity BEFORE UPDATE OF last_seen_at ON main.sessions BEGIN SELECT RAISE(ABORT,'fixture'); END;")).unwrap();
    assert!(
        db.flush_session_activity(std::time::Instant::now(), true)
            .is_err()
    );
    assert_eq!(
        db.session_get(&row.client_id)
            .unwrap()
            .unwrap()
            .last_seen_at
            .as_deref(),
        Some("2026-03-01T00:00:00.000Z")
    );
    db.with_conn(|c| c.execute_batch("DROP TRIGGER reject_activity"))
        .unwrap();
    assert!(
        db.flush_session_activity(std::time::Instant::now(), true)
            .unwrap()
    );
    db.session_touch(&[(row.client_id.clone(), "2026-04-01T00:00:00.000Z".into())])
        .unwrap();
    db.session_delete(&row.client_id).unwrap();
    row.token = "rotated-token".into();
    db.session_save(&row).unwrap();
    assert!(
        !db.flush_session_activity(std::time::Instant::now(), true)
            .unwrap()
    );
    assert_eq!(
        db.session_get(&row.client_id)
            .unwrap()
            .unwrap()
            .last_seen_at,
        None
    );
}

#[test]
fn raw_session_changes_merge_activity_only_for_the_same_credentials() {
    let db = test_db("raw-buffered-session-activity");
    let row = session_cache_row();
    db.session_save(&row).unwrap();
    db.session_touch(&[(row.client_id.clone(), "2026-03-01T00:00:00.000Z".into())])
        .unwrap();
    db.with_conn(|c| {
        c.execute(
            "UPDATE sessions SET name='renamed' WHERE client_id=?1",
            [&row.client_id],
        )
    })
    .unwrap();
    let cached = db.session_get(&row.client_id).unwrap().unwrap();
    assert_eq!(cached.name, "renamed");
    assert_eq!(
        cached.last_seen_at.as_deref(),
        Some("2026-03-01T00:00:00.000Z")
    );
    db.with_conn(|c| {
        c.execute(
            "UPDATE sessions SET name='raw-change' WHERE client_id=?1",
            [&row.client_id],
        )
    })
    .unwrap();
    db.session_save(&row).unwrap();
    assert!(
        !db.flush_session_activity(std::time::Instant::now(), true)
            .unwrap()
    );
    db.session_touch(&[(row.client_id.clone(), "2026-03-01T00:00:00.000Z".into())])
        .unwrap();
    db.with_conn(|c| {
        c.execute(
            "UPDATE sessions SET token='rotated' WHERE client_id=?1",
            [&row.client_id],
        )
    })
    .unwrap();
    assert_eq!(
        db.session_get(&row.client_id)
            .unwrap()
            .unwrap()
            .last_seen_at,
        None
    );
    assert!(
        !db.flush_session_activity(std::time::Instant::now(), true)
            .unwrap()
    );
}

#[test]
fn session_identity_reads_and_new_activity_do_not_wait_for_disk_commit() {
    let db = test_db("nonblocking-session-activity");
    let row = session_cache_row();
    db.session_save(&row).unwrap();
    db.session_touch(&[(row.client_id.clone(), "2026-03-01T00:00:00.000Z".into())])
        .unwrap();
    let (started, waiting) = std::sync::mpsc::sync_channel(1);
    let (release, commit) = std::sync::mpsc::sync_channel(1);
    db.with_conn(|c| {
        c.commit_hook(Some(move || {
            started.send(()).unwrap();
            commit.recv().unwrap();
            false
        }))
    });
    let writer = db.clone();
    let write =
        std::thread::spawn(move || writer.flush_session_activity(std::time::Instant::now(), true));
    waiting.recv().unwrap();
    let available = db.1.sessions.try_read().is_ok();
    let cached = available && db.cached_sessions().is_some();
    if cached {
        assert_eq!(
            db.session_get(&row.client_id).unwrap().unwrap().token,
            row.token
        );
        db.session_touch(&[(row.client_id.clone(), "2026-04-01T00:00:00.000Z".into())])
            .unwrap();
        assert_eq!(
            db.session_list().unwrap()[0].last_seen_at.as_deref(),
            Some("2026-04-01T00:00:00.000Z")
        );
    }
    release.send(()).unwrap();
    assert!(write.join().unwrap().unwrap());
    db.with_conn(|c| c.commit_hook(None::<fn() -> bool>));
    assert!(
        cached,
        "session memory must remain available while SQLite commit is blocked"
    );
    assert_eq!(
        db.session_get(&row.client_id)
            .unwrap()
            .unwrap()
            .last_seen_at
            .as_deref(),
        Some("2026-04-01T00:00:00.000Z")
    );
    assert!(
        db.flush_session_activity(std::time::Instant::now(), true)
            .unwrap()
    );
    assert_eq!(
        db.with_conn(|c| c.query_row(
            "SELECT last_seen_at FROM sessions WHERE client_id=?1",
            [&row.client_id],
            |r| r.get::<_, String>(0)
        ))
        .unwrap(),
        "2026-04-01T00:00:00.000Z"
    );
}
