use super::*;

fn seed(db: &Db, path: &Path) -> String {
    seed_kind(db, path, DataType::Doc)
}

fn seed_kind(db: &Db, path: &Path, kind: DataType) -> String {
    db.record_media_files(&[plain_rs::db::MediaFileMetadata {
        album: String::new(),
        album_artist: String::new(),

        data_type: kind,
        path: path.to_string_lossy().into_owned(),
        title: "document".into(),
        artist: String::new(),
        bucket_id: path.parent().unwrap().to_string_lossy().into_owned(),
        size_bytes: 8,
        duration_ms: 0,
        cover_image_id: None,
        width: 0,
        height: 0,
        rotation: 0,
        taken_at: None,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    }])
    .unwrap()[0]
        .id
        .clone()
}

#[cfg(feature = "runtime-media")]
fn scans() -> Arc<super::super::media_scans::MediaScans> {
    Arc::new(super::super::media_scans::MediaScans::new(
        tokio::sync::broadcast::channel(8).0,
        std::env::temp_dir().join("plain-media-scan-cache"),
    ))
}

#[test]
fn native_move_runs_one_commit_after_the_physical_move_and_preserves_collisions() {
    let directory = tempfile::tempdir().unwrap();
    let source_dir = directory.path().join("source");
    let target = directory.path().join("target");
    std::fs::create_dir(&source_dir).unwrap();
    std::fs::create_dir(&target).unwrap();
    let source = source_dir.join("note.txt");
    std::fs::write(&source, b"document").unwrap();
    std::fs::write(target.join("note.txt"), b"keep original").unwrap();
    let db = Db::open(&directory.path().join("plain.db")).unwrap();
    let id = seed(&db, &source);
    let operation = Operation {
        kind: DataType::Doc,
        action: Action::Move,
        ids: vec![id.clone()],
        backend: Backend::Files {
            from_trash: false,
            destination: target.to_string_lossy().into_owned(),
            #[cfg(feature = "runtime-media")]
            scans: scans(),
        },
    };
    let count = std::sync::atomic::AtomicUsize::new(0);
    operation
        .complete(&db, |outcome| {
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            assert_eq!(outcome.successful.len(), 1);
            assert!(!source.exists());
            assert_eq!(
                std::fs::read(target.join("note (1).txt")).unwrap(),
                b"document"
            );
            assert_eq!(
                db.with_conn(|connection| connection.query_row(
                    "SELECT path FROM file_catalog WHERE id=?1",
                    [&id],
                    |row| row.get::<_, String>(0)
                ))
                .unwrap(),
                source
                    .parent()
                    .unwrap()
                    .canonicalize()
                    .unwrap()
                    .join(source.file_name().unwrap())
                    .to_string_lossy()
            );
            plain_rs::media::actions::cleanup(
                &db,
                DataType::Doc,
                Action::Move,
                &outcome.successful,
            )?;
            Ok(())
        })
        .unwrap();
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(
        std::fs::read(target.join("note.txt")).unwrap(),
        b"keep original"
    );
    assert_eq!(
        db.with_conn(|connection| connection.query_row(
            "SELECT path FROM file_catalog WHERE id=?1",
            [&id],
            |row| row.get::<_, String>(0)
        ))
        .unwrap(),
        target
            .join("note (1).txt")
            .canonicalize()
            .unwrap()
            .to_string_lossy()
    );
}

#[cfg(feature = "runtime-media")]
#[test]
fn a_closed_scanner_refuses_physical_work_and_commit() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("note.txt");
    std::fs::write(&path, b"document").unwrap();
    let db = Db::open(&directory.path().join("plain.db")).unwrap();
    let id = seed(&db, &path);
    let scans = scans();
    scans.close().unwrap();
    let operation = Operation {
        kind: DataType::Doc,
        action: Action::Delete,
        ids: vec![id],
        backend: Backend::Files {
            from_trash: false,
            destination: String::new(),
            scans,
        },
    };
    assert!(
        operation
            .complete(&db, |_| -> DomainResult<()> {
                panic!("closed core must not commit")
            })
            .is_err()
    );
    assert!(path.exists());
}

#[test]
fn invalid_provider_receipts_never_reach_shared_cleanup() {
    let directory = tempfile::tempdir().unwrap();
    let db = Db::open(&directory.path().join("plain.db")).unwrap();
    let operation = Operation {
        kind: DataType::Doc,
        action: Action::Delete,
        ids: vec!["requested".into()],
        backend: Backend::Provider(Outcome {
            successful: vec![Item {
                id: "other".into(),
                path: "/other".into(),
                destination_path: String::new(),
            }],
            failed_ids: Vec::new(),
        }),
    };
    assert!(
        operation
            .complete(&db, |_| -> DomainResult<()> {
                panic!("invalid receipt must not commit")
            })
            .is_err()
    );
}

#[test]
fn physical_failure_preserves_queue_and_tag_references_for_only_the_failed_row() {
    let directory = tempfile::tempdir().unwrap();
    let db = Db::open(&directory.path().join("plain.db")).unwrap();
    let good_path = directory.path().join("good.wav");
    let blocked = directory.path().join("blocked");
    std::fs::write(&good_path, b"document").unwrap();
    std::fs::create_dir(&blocked).unwrap();
    let good = seed_kind(&db, &good_path, DataType::Audio);
    let bad = seed_kind(&db, &blocked.join("missing.wav"), DataType::Audio);
    std::fs::remove_dir(&blocked).unwrap();
    std::fs::write(&blocked, b"not a directory").unwrap();
    db.with_conn(|connection| -> rusqlite::Result<()> {
        connection.execute_batch("INSERT INTO tags(id,name,type,created_at,updated_at) VALUES('audio','audio',1,'now','now'),('video','video',2,'now','now');")?;
        for (index, id) in [&good, &bad].into_iter().enumerate() {
            connection.execute("INSERT INTO audio_queue_items(file_id,sort_order) VALUES(?1,?2)", rusqlite::params![id,index as i64])?;
            connection.execute("INSERT INTO tag_relations(tag_id,target_key,type,created_at) VALUES('audio',?1,1,'now'),('video',?1,2,'now')", [id])?;
        }
        Ok(())
    }).unwrap();
    let operation = Operation {
        kind: DataType::Audio,
        action: Action::Delete,
        ids: vec![good.clone(), bad.clone()],
        backend: Backend::Files {
            from_trash: false,
            destination: String::new(),
            #[cfg(feature = "runtime-media")]
            scans: scans(),
        },
    };
    let outcome = operation
        .complete(&db, |outcome| {
            plain_rs::media::actions::cleanup(
                &db,
                DataType::Audio,
                Action::Delete,
                &outcome.successful,
            )?;
            Ok(outcome)
        })
        .unwrap();
    assert_eq!(outcome.successful.len(), 1);
    assert_eq!(outcome.failed_ids, vec![bad.clone()]);
    assert!(!good_path.exists());
    db.with_conn(|connection| -> rusqlite::Result<()> {
        assert_eq!(
            connection.query_row("SELECT file_id FROM audio_queue_items", [], |row| row
                .get::<_, String>(0))?,
            bad
        );
        assert_eq!(
            connection.query_row(
                "SELECT count(*) FROM tag_relations WHERE type=1",
                [],
                |row| row.get::<_, i64>(0)
            )?,
            1
        );
        assert_eq!(
            connection.query_row(
                "SELECT count(*) FROM tag_relations WHERE type=2",
                [],
                |row| row.get::<_, i64>(0)
            )?,
            2
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn wrong_media_type_and_corrupt_catalog_rows_never_delete_source_files() {
    let directory = tempfile::tempdir().unwrap();
    let db = Db::open(&directory.path().join("plain.db")).unwrap();
    let path = directory.path().join("note.txt");
    std::fs::write(&path, b"document").unwrap();
    let id = seed(&db, &path);
    let outcome = files(
        &db,
        DataType::Audio,
        Action::Delete,
        &[id.clone()],
        false,
        "",
    )
    .unwrap();
    assert_eq!(outcome.failed_ids, vec![id.clone()]);
    assert!(path.exists());
    db.with_conn(|connection| -> rusqlite::Result<()> {
        assert!(
            connection
                .execute("UPDATE files SET path=X'FF' WHERE id=?1", [&id])
                .is_err()
        );
        Ok(())
    })
    .unwrap();
    assert!(
        db.media_file_by_path(DataType::Doc, path.to_str().unwrap())
            .unwrap()
            .is_some()
    );
    assert!(path.exists());
}

#[tokio::test]
async fn native_media_support_is_declared_by_its_adapter_without_a_host_exchange() {
    let host = Host::default();
    let supported = capability_list(&host).await.unwrap();
    assert!(
        plain_rs::services::capabilities::require_list(
            &supported,
            action_capability(Action::Delete)
        )
        .is_ok()
    );
    assert!(
        plain_rs::services::capabilities::require_list(&supported, action_capability(Action::Move))
            .is_ok()
    );
    assert!(
        plain_rs::services::capabilities::require_list(
            &supported,
            action_capability(Action::Trash)
        )
        .is_err()
    );
}
