use super::*;

fn add(connection: &Connection, id: &str, kind: i32, trashed: bool) {
    let id = id.parse::<i64>().unwrap();
    connection.execute("INSERT OR IGNORE INTO volumes(id,identity,path,is_available) VALUES(1,'TEST','/catalog',1)",[]).unwrap();
    connection.execute("INSERT INTO files(id,volume_id,path,file_name,kind,size_bytes,width,height,created_at,updated_at,trashed_at,original_path) VALUES(?1,1,?2,?2,?3,1,1,1,'2026-01-01T00:00:00Z','2026-01-01T00:00:00Z',CASE WHEN ?4 THEN '2026-01-01T00:00:00Z' END,CASE WHEN ?4 THEN ?2 END)",params![id,format!("{id}.png"),if kind==3 {"IMAGE"} else {"AUDIO"},trashed]).unwrap();
}

#[test]
fn native_pages_are_bounded_keyset_pages_of_live_images_and_resolve_current_ids() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    db.with_conn(|connection| {
        for n in (1..=256).rev() {
            add(connection, &n.to_string(), 3, false);
        }
        add(connection, "258", 1, false);
        add(connection, "257", 3, true);
    });
    let catalog = snapshot(&db).unwrap();
    assert_eq!(catalog.total, 256);
    let first = page(&db, &catalog.revision, "", 128).unwrap();
    assert_eq!(first.items.len(), 128);
    assert_eq!(first.items[0].id, "1");
    assert_eq!(first.next_cursor, "128");
    assert!(!first.done);
    let second = page(&db, &catalog.revision, &first.next_cursor, 128).unwrap();
    assert_eq!(second.items.len(), 128);
    assert_eq!(second.items[0].id, "129");
    assert_eq!(second.next_cursor, "256");
    let end = page(&db, &catalog.revision, &second.next_cursor, 128).unwrap();
    assert!(end.done);
    assert!(end.items.is_empty());
    let selected = resolve(
        &db,
        &catalog.revision,
        &[
            "256".into(),
            "257".into(),
            "999".into(),
            "258".into(),
            "1".into(),
        ],
    )
    .unwrap();
    assert_eq!(
        selected
            .iter()
            .map(|image| image.id.as_str())
            .collect::<Vec<_>>(),
        ["1", "256"]
    );
    assert!(page(&db, &catalog.revision, "", 0).is_err());
    assert!(page(&db, &catalog.revision, "", 129).is_err());
    assert!(resolve(&db, &catalog.revision, &vec!["1".into(); 101]).is_err());
}

#[test]
fn catalog_writes_invalidate_revision_but_embedding_writes_do_not_invalidate_the_scan() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    db.with_conn(|connection| add(connection, "1", 3, false));
    let original = snapshot(&db).unwrap();
    crate::media::images::embeddings::save(
        &db,
        &[crate::media::images::embeddings::EmbeddingInput {
            id: "1".into(),
            path: "/catalog/1.png".into(),
            embedding_base64: crate::utils::base64::base64_encode(&1.0f32.to_be_bytes()),
        }],
    )
    .unwrap();
    verify(&db, &original.revision).unwrap();
    db.with_conn(|connection| {
        connection.execute("UPDATE files SET path='changed.png' WHERE id=1", [])
    })
    .unwrap();
    assert!(verify(&db, &original.revision).is_err());
    assert!(page(&db, &original.revision, "", 128).is_err());
    assert!(resolve(&db, &original.revision, &["1".into()]).is_err());
    let current = snapshot(&db).unwrap();
    assert_eq!(
        resolve(&db, &current.revision, &["1".into()]).unwrap()[0].path,
        "/catalog/changed.png"
    );
}

#[test]
fn commits_from_another_connection_invalidate_before_more_rows_or_pruning_are_accepted() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("catalog.db");
    let db = Db::open(&path).unwrap();
    db.with_conn(|connection| add(connection, "1", 3, false));
    let original = snapshot(&db).unwrap();
    let external = Connection::open(&path).unwrap();
    add(&external, "2", 3, false);
    assert!(verify(&db, &original.revision).is_err());
    assert!(page(&db, &original.revision, "", 128).is_err());
    let current = snapshot(&db).unwrap();
    assert_eq!(current.total, 2);
    external
        .execute("DELETE FROM files WHERE id=1", [])
        .unwrap();
    assert!(verify(&db, &current.revision).is_err());
    assert_eq!(snapshot(&db).unwrap().total, 1);
}

#[test]
fn keyset_page_uses_the_catalog_order_index_without_a_temporary_sort() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let plan = db.with_conn(|connection| {
        connection.prepare("EXPLAIN QUERY PLAN SELECT id,path FROM files WHERE kind=?1 AND missing=0 AND trashed_at IS NULL AND id>?2 ORDER BY id LIMIT ?3").unwrap()
            .query_map(params!["IMAGE",0,128], |row| row.get::<_,String>(3)).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap()
    });
    assert!(
        plan.iter().any(|step| step.contains("idx_files_kind_id")),
        "{plan:?}"
    );
    assert!(
        !plan.iter().any(|step| step.contains("TEMP B-TREE")),
        "{plan:?}"
    );
}

#[test]
fn rollback_noop_and_other_media_updates_do_not_invalidate_an_image_pass() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    db.with_conn(|connection| {
        add(connection, "1", 3, false);
        add(connection, "258", 1, false);
    });
    let original = snapshot(&db).unwrap();
    db.with_conn(|connection| {
        connection
            .execute("UPDATE files SET path='changed.mp3' WHERE id=258", [])
            .unwrap();
        connection
            .execute("UPDATE files SET updated_at=updated_at WHERE id=1", [])
            .unwrap();
        let transaction = connection.unchecked_transaction().unwrap();
        transaction
            .execute("DELETE FROM files WHERE id=1", [])
            .unwrap();
        transaction.rollback().unwrap();
    });
    verify(&db, &original.revision).unwrap();
    assert_eq!(
        page(&db, &original.revision, "", 128).unwrap().items[0].id,
        "1"
    );
}
