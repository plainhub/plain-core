use super::*;
#[path = "../library/fixtures.rs"]
mod fixtures;
use fixtures::test_db;
fn item(id: &str, path: &str) -> Item {
    Item {
        id: id.into(),
        path: path.into(),
        destination_path: String::new(),
    }
}
#[test]
fn only_acknowledged_successes_are_cleaned_and_type_isolated() {
    let db = test_db("media_action_success");
    db.with_conn(|c|c.execute_batch("INSERT INTO tags(id,name,type,created_at,updated_at) VALUES('audio','audio',1,'now','now'),('video','video',2,'now','now'); INSERT INTO tag_relations(tag_id,target_key,type,created_at) VALUES('audio','one',1,'now'),('audio','failed',1,'now'),('video','one',2,'now'); INSERT INTO platform_audio_media(file_id,path,title,artist,duration_ms,album_id) VALUES('test:/one','/one','one','',123,''),('test:/failed','/failed','failed','',123,''); INSERT INTO audio_queue_items(file_id,sort_order) VALUES('test:/one',0),('test:/failed',1);")).unwrap();
    let outcome = Outcome {
        successful: vec![item("one", "/one")],
        failed_ids: vec!["failed".into()],
    };
    validate(
        DataType::Audio,
        Action::Delete,
        &["one".into(), "failed".into()],
        &outcome,
    )
    .unwrap();
    assert_eq!(
        cleanup(&db, DataType::Audio, Action::Delete, &outcome.successful).unwrap(),
        1
    );
    db.with_conn(|c| {
        assert_eq!(
            c.query_row("SELECT count(*) FROM tag_relations", [], |r| r
                .get::<_, i64>(0))?,
            2
        );
        assert_eq!(
            c.query_row(
                "SELECT m.path FROM audio_queue_items q JOIN audio_media m USING(file_id)",
                [],
                |r| r.get::<_, String>(0)
            )?,
            "/failed"
        );
        Ok::<_, rusqlite::Error>(())
    })
    .unwrap();
}
#[test]
fn malformed_or_incomplete_receipts_are_rejected() {
    let requested = vec!["one".into(), "two".into()];
    let mut outcome = Outcome {
        successful: vec![item("one", "/one")],
        failed_ids: vec![],
    };
    assert!(validate(DataType::Image, Action::Delete, &requested, &outcome).is_err());
    outcome.failed_ids.push("two".into());
    validate(DataType::Image, Action::Delete, &requested, &outcome).unwrap();
    outcome.successful.push(item("alien", "/alien"));
    assert!(validate(DataType::Image, Action::Delete, &requested, &outcome).is_err());
    assert!(validate(DataType::Note, Action::Delete, &requested, &outcome).is_err());
}
#[test]
fn cleanup_transaction_failure_preserves_all_references() {
    let db = test_db("media_action_rollback");
    db.with_conn(|c|c.execute_batch("INSERT INTO tags(id,name,type,created_at,updated_at) VALUES('audio','audio',1,'now','now'); INSERT INTO tag_relations(tag_id,target_key,type,created_at) VALUES('audio','one',1,'now'); INSERT INTO platform_audio_media(file_id,path,title,artist,duration_ms,album_id) VALUES('test:/one','/one','one','',123,''); INSERT INTO audio_queue_items(file_id,sort_order) VALUES('test:/one',0); CREATE TRIGGER reject_queue BEFORE DELETE ON audio_queue_items BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;")).unwrap();
    assert!(cleanup(&db, DataType::Audio, Action::Delete, &[item("one", "/one")]).is_err());
    db.with_conn(|c| {
        assert_eq!(
            c.query_row("SELECT count(*) FROM tag_relations", [], |r| r
                .get::<_, i64>(0))?,
            1
        );
        assert_eq!(
            c.query_row("SELECT count(*) FROM audio_queue_items", [], |r| r
                .get::<_, i64>(0))?,
            1
        );
        Ok::<_, rusqlite::Error>(())
    })
    .unwrap();
}

#[test]
fn video_cleanup_keeps_other_types_and_move_keeps_user_tags() {
    let db = test_db("media_action_video_image");
    db.with_conn(|c|c.execute_batch("INSERT INTO tags(id,name,type,created_at,updated_at) VALUES('image','image',3,'now','now'); INSERT INTO tag_relations(tag_id,target_key,type,created_at) VALUES('image','one',3,'now'); INSERT INTO media_play_history(media_type,file_id,position_ms,played_at,play_count) VALUES(2,'one',456,'now',0); INSERT INTO image_embeddings(file_id,path,embedding,created_at,updated_at) VALUES('one','/one',X'3f800000','now','now');")).unwrap();
    let item = Item {
        id: "one".into(),
        path: "/one".into(),
        destination_path: "/new/one".into(),
    };
    cleanup(
        &db,
        DataType::Image,
        Action::Move,
        std::slice::from_ref(&item),
    )
    .unwrap();
    db.with_conn(|c| {
        assert_eq!(
            c.query_row("SELECT count(*) FROM tag_relations", [], |r| r
                .get::<_, i64>(0))?,
            1
        );
        assert_eq!(
            c.query_row("SELECT count(*) FROM image_embeddings", [], |r| r
                .get::<_, i64>(0))?,
            0
        );
        Ok::<_, rusqlite::Error>(())
    })
    .unwrap();
    cleanup(
        &db,
        DataType::Video,
        Action::Delete,
        std::slice::from_ref(&item),
    )
    .unwrap();
    db.with_conn(|c| {
        assert_eq!(
            c.query_row("SELECT count(*) FROM tag_relations", [], |r| r
                .get::<_, i64>(0))?,
            1
        );
        assert_eq!(
            c.query_row("SELECT count(*) FROM media_play_history", [], |r| r
                .get::<_, i64>(0))?,
            0
        );
        Ok::<_, rusqlite::Error>(())
    })
    .unwrap();
}

fn catalog_file(path: &str) -> crate::db::MediaFileMetadata {
    crate::db::MediaFileMetadata {
        album: String::new(),
        album_artist: String::new(),

        data_type: DataType::Doc,
        path: path.into(),
        title: "document".into(),
        artist: String::new(),
        bucket_id: "/library".into(),
        size_bytes: 10,
        duration_ms: 0,
        cover_image_id: None,
        width: 0,
        height: 0,
        rotation: 0,
        taken_at: None,
        created_at: "2026-01-01T00:00:00.000Z".into(),
        updated_at: "2026-01-01T00:00:00.000Z".into(),
    }
}

#[test]
fn confirmed_actions_preserve_catalog_identity_and_restore_the_original_path() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let path = |relative: &str| root.join(relative).to_str().unwrap().to_owned();
    let db = test_db("catalog_action_state");
    let original = db
        .record_media_files(&[catalog_file(&path("library/document.txt"))])
        .unwrap()
        .remove(0);
    db.with_conn(|conn| conn.execute("UPDATE files SET favorited=1 WHERE id=?1", [&original.id]))
        .unwrap();
    let mut receipt = item(&original.id, &original.path);
    receipt.destination_path = path(".plain-trash/document.txt");
    cleanup(&db, DataType::Doc, Action::Trash, &[receipt.clone()]).unwrap();
    let trash = db
        .media_file_by_path(DataType::Doc, &receipt.destination_path)
        .unwrap()
        .unwrap();
    assert!(trash.trashed);
    assert!(trash.favorited);
    assert_eq!(trash.id, original.id);
    assert_eq!(trash.original_path, Some(original.path.clone()));
    cleanup(
        &db,
        DataType::Doc,
        Action::Restore,
        &[item(&trash.id, &trash.path)],
    )
    .unwrap();
    let restored = db
        .media_file_by_path(DataType::Doc, &original.path)
        .unwrap()
        .unwrap();
    assert!(!restored.trashed);
    assert_eq!(restored.original_path, None);
    assert_eq!(restored.created_at, original.created_at);
    receipt.path = restored.path;
    receipt.destination_path = path("other/renamed.pdf");
    cleanup(&db, DataType::Doc, Action::Move, &[receipt.clone()]).unwrap();
    let moved = db
        .media_file_by_path(DataType::Doc, &receipt.destination_path)
        .unwrap()
        .unwrap();
    assert_eq!(moved.id, original.id);
    assert_eq!(moved.bucket_id, path("other"));
    assert!(moved.favorited);
    cleanup(
        &db,
        DataType::Doc,
        Action::Delete,
        &[item(&moved.id, &moved.path)],
    )
    .unwrap();
    assert!(
        db.media_file_by_path(DataType::Doc, &moved.path)
            .unwrap()
            .is_none()
    );
}

#[test]
fn stale_receipts_and_catalog_sql_failures_do_not_commit_partial_updates() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let path = |relative: &str| root.join(relative).to_str().unwrap().to_owned();
    let db = test_db("catalog_action_atomic");
    let files = db
        .record_media_files(&[
            catalog_file(&path("library/one.txt")),
            catalog_file(&path("library/two.txt")),
        ])
        .unwrap();
    assert!(
        cleanup(
            &db,
            DataType::Doc,
            Action::Delete,
            &[item(&files[0].id, "/wrong/path")]
        )
        .is_err()
    );
    let mut first = item(&files[0].id, &files[0].path);
    first.destination_path = path(".plain-trash/first.txt");
    assert!(
        cleanup(
            &db,
            DataType::Doc,
            Action::Trash,
            &[first, item(&files[1].id, "/wrong/path")]
        )
        .is_err()
    );
    assert!(
        !db.media_file_by_path(DataType::Doc, &files[0].path)
            .unwrap()
            .unwrap()
            .trashed
    );
    db.with_conn(|conn| conn.execute_batch("CREATE TRIGGER reject_catalog_delete BEFORE DELETE ON files BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;")).unwrap();
    assert!(
        cleanup(
            &db,
            DataType::Doc,
            Action::Delete,
            &[item(&files[0].id, &files[0].path)]
        )
        .is_err()
    );
    assert!(
        db.media_file_by_path(DataType::Doc, &files[0].path)
            .unwrap()
            .is_some()
    );
    db.with_conn(|conn| conn.execute_batch("DROP TRIGGER reject_catalog_delete;"))
        .unwrap();
    let mut conflict = item(&files[0].id, &files[0].path);
    conflict.destination_path = files[1].path.clone();
    assert!(cleanup(&db, DataType::Doc, Action::Move, &[conflict]).is_err());
    assert!(
        db.media_file_by_path(DataType::Doc, &files[0].path)
            .unwrap()
            .is_some()
    );
}
