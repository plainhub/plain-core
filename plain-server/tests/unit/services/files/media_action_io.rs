use super::*;

#[cfg(feature = "media")]
#[tokio::test]
async fn existing_media_delete_filters_trash_and_preserves_unselected_files() {
    let directory = tempfile::tempdir().unwrap();
    let core =
        crate::assembly::server::ContentServer::open(&directory.path().join("core")).unwrap();
    let catalog = core.native_media_catalog().unwrap();
    let paths = [
        directory.path().join("one.txt"),
        directory.path().join("two.txt"),
    ];
    let metadata = paths
        .iter()
        .map(|path| {
            std::fs::write(path, b"document").unwrap();
            plain_rs::db::MediaFileMetadata {
                album: String::new(),
                album_artist: String::new(),

                data_type: plain_rs::enums::DataType::Doc,
                path: path.to_str().unwrap().into(),
                title: "document".into(),
                artist: String::new(),
                bucket_id: directory.path().to_str().unwrap().into(),
                size_bytes: 8,
                duration_ms: 0,
                cover_image_id: None,
                width: 0,
                height: 0,
                rotation: 0,
                taken_at: None,
                created_at: "2026-01-01T00:00:00.000Z".into(),
                updated_at: "2026-01-01T00:00:00.000Z".into(),
            }
        })
        .collect::<Vec<_>>();
    let rows = catalog.record_files(&metadata).unwrap();
    core.runtime_state()
        .db
        .with_conn(|conn| {
            conn.execute(
                "UPDATE files SET trashed_at='2026-01-01T00:00:00Z',original_path=path WHERE id=?1",
                [&rows[1].id],
            )
        })
        .unwrap();
    let ids = rows.iter().map(|row| row.id.clone()).collect::<Vec<_>>();
    let native = core.native_graphql().unwrap();
    let response = native.execute(async_graphql::Request::new("mutation Delete($query: String!) { deleteMediaItems(type: DOC, query: $query) { affectedCount } }")
        .variables(async_graphql::Variables::from_json(serde_json::json!({"query":format!("ids:{} trash:false",ids.join(","))})))).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["deleteMediaItems"]["affectedCount"], 1);
    assert!(!paths[0].exists());
    assert!(paths[1].exists());
    assert!(
        catalog
            .by_path(plain_rs::enums::DataType::Doc, paths[0].to_str().unwrap())
            .unwrap()
            .is_none()
    );
    assert!(
        catalog
            .by_path(plain_rs::enums::DataType::Doc, paths[1].to_str().unwrap())
            .unwrap()
            .is_some()
    );
    core.stop_core().await;
    let response = native
        .execute(async_graphql::Request::new("{ __typename }"))
        .await;
    assert!(
        !response.errors.is_empty(),
        "stopped native executor must reject requests"
    );
}

#[cfg(feature = "media")]
#[tokio::test]
async fn image_delete_updates_index_before_any_parent_consumes_the_receipt() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("photo.jpg");
    std::fs::write(&path, b"image").unwrap();
    let core =
        crate::assembly::server::ContentServer::open(&directory.path().join("core")).unwrap();
    let catalog = core.native_media_catalog().unwrap();
    let rows = catalog
        .record_files(&[plain_rs::db::MediaFileMetadata {
            album: String::new(),
            album_artist: String::new(),

            data_type: plain_rs::enums::DataType::Image,
            path: path.to_str().unwrap().into(),
            title: "photo".into(),
            artist: String::new(),
            bucket_id: directory.path().to_str().unwrap().into(),
            size_bytes: 5,
            duration_ms: 0,
            cover_image_id: None,
            width: 0,
            height: 0,
            rotation: 0,
            taken_at: None,
            created_at: "2026-01-01T00:00:00.000Z".into(),
            updated_at: "2026-01-01T00:00:00.000Z".into(),
        }])
        .unwrap();
    let index = core.runtime_state().image_index.clone();
    let version = index.status().version;
    let response = core.native_graphql().unwrap().execute(async_graphql::Request::new("mutation Delete($query: String!) { deleteMediaItems(type: IMAGE, query: $query) { affectedCount } }")
        .variables(async_graphql::Variables::from_json(serde_json::json!({"query":format!("ids:{} trash:false",rows[0].id)})))).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["deleteMediaItems"]["affectedCount"],
        1
    );
    assert!(!path.exists());
    assert_eq!(catalog.count(plain_rs::enums::DataType::Image).unwrap(), 0);
    assert_eq!(
        index.status().version,
        version + 1,
        "image index cleanup runs exactly once"
    );
    assert_eq!(index.status().indexed_images, 0);
    core.stop_core().await;
}

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

#[cfg(feature = "media")]
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
            #[cfg(feature = "media")]
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

#[cfg(feature = "media")]
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

#[cfg(feature = "media")]
#[tokio::test]
async fn native_main_delete_has_no_host_exchange_and_publishes_each_event_once() {
    let directory = tempfile::tempdir().unwrap();
    let core =
        crate::assembly::server::ContentServer::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state();
    let source = directory.path().join("note.txt");
    std::fs::write(&source, b"document").unwrap();
    let id = seed(&state.db, &source);
    let (_generation, mut requests) = state.host.connect();
    let mut events = state.events.subscribe();
    let response = core.native_graphql().unwrap().execute(async_graphql::Request::new("mutation Delete($query: String!) { deleteMediaItems(type: DOC, query: $query) { affectedCount } }").variables(async_graphql::Variables::from_json(serde_json::json!({"query":format!("ids:{id} trash:false")})))).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert!(requests.try_recv().is_err());
    let mut content = 0;
    while let Ok(event) = events.try_recv() {
        match event.event_type {
            "CONTENT_CHANGED" => content += 1,
            _ => {}
        }
    }
    assert_eq!(content, 1);
    assert!(!source.exists());
    core.stop_core().await;
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
            #[cfg(feature = "media")]
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
        crate::services::capabilities::require_list(&supported, action_capability(Action::Delete))
            .is_ok()
    );
    assert!(
        crate::services::capabilities::require_list(&supported, action_capability(Action::Move))
            .is_ok()
    );
    assert!(
        crate::services::capabilities::require_list(&supported, action_capability(Action::Trash))
            .is_err()
    );
}

#[cfg(feature = "media")]
#[tokio::test]
async fn partial_main_media_actions_invalidate_only_completed_duration_entries() {
    use plain_rs::media::duration_cache::Duration;
    for (kind, name, media_type) in [
        (DataType::Audio, "AUDIO", "audio"),
        (DataType::Video, "VIDEO", "video"),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let core =
            crate::assembly::server::ContentServer::open(&directory.path().join("core")).unwrap();
        let state = core.runtime_state();
        let (_generation, mut requests) = state.host.connect();
        let good_path = directory.path().join("good.mp4");
        let blocked = directory.path().join("blocked");
        std::fs::write(&good_path, b"document").unwrap();
        std::fs::create_dir(&blocked).unwrap();
        let bad_path = blocked.join("bad.mp4");
        let good = seed_kind(&state.db, &good_path, kind);
        let bad = seed_kind(&state.db, &bad_path, kind);
        std::fs::remove_dir(&blocked).unwrap();
        std::fs::write(&blocked, b"not a directory").unwrap();
        let duration = |id: &str, path: &Path| Duration {
            media_type: media_type.into(),
            file_id: id.into(),
            path: path.to_string_lossy().into_owned(),
            size_bytes: 8,
            modified_at: "2026-01-01T00:00:00Z".into(),
            duration_ms: 5_000_000_001,
            updated_at: String::new(),
        };
        state
            .duration_cache
            .save(duration(&good, &good_path))
            .unwrap();
        state
            .duration_cache
            .save(duration(&bad, &bad_path))
            .unwrap();
        let revision = state.duration_cache.revision();
        let response = core.native_graphql().unwrap().execute(async_graphql::Request::new(format!(
            "mutation Delete($query: String!) {{ deleteMediaItems(type: {name}, query: $query) {{ affectedCount }} }}"
        )).variables(async_graphql::Variables::from_json(serde_json::json!({"query":format!("ids:{good},{bad} trash:false")})))).await;
        assert_eq!(response.errors.len(), 1, "{:?}", response.errors);
        assert!(
            response.errors[0]
                .message
                .contains("1 media actions failed (1 completed)")
        );
        assert!(!good_path.exists());
        assert!(blocked.is_file());
        let remaining = state.duration_cache.all().unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].file_id, bad);
        assert_eq!(remaining[0].duration_ms, 5_000_000_001);
        assert!(
            !state
                .duration_cache
                .save_at_revision(duration(&good, &good_path), revision)
                .unwrap()
        );
        assert_eq!(state.duration_cache.all().unwrap().len(), 1);
        assert!(requests.try_recv().is_err());
        core.stop_core().await;
    }
}
