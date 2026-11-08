use crate::test_runtime::Core as ContentServer;

#[cfg(feature = "runtime-media")]
#[tokio::test]
async fn deleting_an_indexed_image_publishes_its_library_type_and_preserves_its_neighbor() {
    use crate::features::enums::DataType;
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = ContentServer::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state();
    let catalog =
        crate::features::services::files::catalog_scan::NativeMediaCatalog::new(state.clone());
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/v-prog420.jpg"),
    )
    .unwrap();
    let selected = directory.path().join("selected.jpg");
    let neighbor = directory.path().join("neighbor.jpg");
    for path in [&selected, &neighbor] {
        std::fs::write(path, &bytes).unwrap();
        catalog
            .record_files(&[crate::features::media::file_facts::read_file_metadata(
                path,
                DataType::Image,
            )
            .unwrap()])
            .unwrap();
    }
    let mut events = state.events.subscribe();
    assert_eq!(
        state
            .files
            .delete_many(vec![selected.to_string_lossy().into_owned()])
            .await
            .unwrap(),
        1
    );
    let mut changes = Vec::new();
    while let Ok(event) = events.try_recv() {
        if event.event_type == crate::infrastructure::events::MEDIA_LIBRARY_UPDATED {
            changes.push(serde_json::from_str::<serde_json::Value>(&event.payload).unwrap());
        }
    }
    assert!(
        changes.contains(&serde_json::json!({"types":[3]})),
        "{changes:?}"
    );
    assert!(!selected.exists());
    assert_eq!(std::fs::read(&neighbor).unwrap(), bytes);
    assert_eq!(state.db.media_file_count(DataType::Image).unwrap(), 1);
    core.stop_core().await;
}

#[tokio::test]
async fn overwriting_move_ignores_retired_catalog_rows_when_deleting_the_current_file() {
    let directory = crate::test_runtime::public_directory();
    let root = directory.path().canonicalize().unwrap();
    let core = ContentServer::open(&root.join("core")).unwrap();
    let state = core.runtime_state();
    let catalog =
        crate::features::services::files::catalog_scan::NativeMediaCatalog::new(state.clone());
    crate::features::services::files::indexing::set_directories(
        &state.db,
        &[root.to_string_lossy().into_owned()],
    )
    .unwrap();
    catalog.configure_scan_directories(None).unwrap();
    let source = root.join("source.bin");
    let destination = root.join("destination.bin");
    let neighbor = root.join("neighbor.bin");
    for (path, data) in [
        (&source, b"source".as_slice()),
        (&destination, b"old"),
        (&neighbor, b"neighbor"),
    ] {
        std::fs::write(path, data).unwrap();
        state
            .file_platform
            .scan(&[path.to_str().unwrap().into()])
            .await
            .unwrap();
    }
    let paths = vec![
        source.to_str().unwrap().into(),
        destination.to_str().unwrap().into(),
    ];
    let before = state.file_platform.identities(&paths).await.unwrap();
    assert_eq!(before.len(), 2);
    let source_id = before
        .iter()
        .find(|item| item.path == paths[0])
        .unwrap()
        .file_id
        .clone();
    let task = state
        .files
        .execute(
            "fixture",
            crate::features::files::tasks::FileTaskType::Move,
            vec![crate::features::files::tasks::FileTaskOp {
                src: paths[0].clone(),
                dst: paths[1].clone(),
                overwrite: true,
            }],
        )
        .await
        .unwrap();
    assert_eq!(
        task.status,
        crate::features::files::tasks::FileTaskStatus::Done
    );
    assert_eq!(std::fs::read(&destination).unwrap(), b"source");
    assert!(!source.exists());
    let current = state
        .file_platform
        .identities(&[paths[1].clone()])
        .await
        .unwrap();
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].file_id, source_id);
    assert_eq!(
        state
            .files
            .delete_many(vec![paths[1].clone()])
            .await
            .unwrap(),
        1
    );
    assert!(!destination.exists());
    assert_eq!(std::fs::read(&neighbor).unwrap(), b"neighbor");
    core.stop_core().await;
}

fn relation(core: &ContentServer, tag: &str) -> Option<String> {
    core.runtime_state()
        .db
        .with_conn(|connection| {
            use rusqlite::OptionalExtension;
            connection
                .query_row(
                    "SELECT target_key FROM tag_relations WHERE tag_id=?1",
                    [tag],
                    |row| row.get(0),
                )
                .optional()
        })
        .unwrap()
}

#[cfg(feature = "runtime-media")]
#[tokio::test]
async fn deletion_cleans_catalog_without_reindexing_missing_files_and_repeat_is_empty() {
    let directory = crate::test_runtime::public_directory();
    let core = ContentServer::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state();
    let source = directory.path().join("source.txt");
    std::fs::write(&source, b"fixture").unwrap();
    state.db.with_conn(|connection| connection.execute(
        "INSERT INTO tag_relations(tag_id,target_key,type,created_at) VALUES('file',?1,22,'first')", [source.to_str().unwrap()]
    )).unwrap();
    state.media_scans.close().unwrap();
    let later = directory.path().join("later.txt");
    std::fs::write(&later, b"later").unwrap();
    let first = state
        .files
        .delete_many(vec![
            source.to_str().unwrap().into(),
            later.to_str().unwrap().into(),
        ])
        .await;
    assert_eq!(first.unwrap(), 2);
    assert!(!source.exists());
    assert!(!later.exists());
    assert!(relation(&core, "file").is_none());
    let repeated = state
        .files
        .delete(source.to_str().unwrap().into())
        .await
        .unwrap();
    assert!(!repeated.removed);
    assert!(repeated.paths.is_empty());
    assert!(
        state
            .files
            .list(state.native_client_id().unwrap())
            .await
            .unwrap()
            .is_empty()
    );
    core.stop_core().await;
}

#[tokio::test]
async fn public_rename_rejects_traversal_and_keeps_the_original_file() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = ContentServer::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state();
    let source = directory.path().join("source");
    std::fs::write(&source, b"source").unwrap();
    for name in ["../outside", "/outside", "", ".", ".."] {
        assert!(
            state
                .files
                .rename(
                    "rename-client",
                    source.to_string_lossy().into_owned(),
                    name.into()
                )
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(std::fs::read(&source).unwrap(), b"source");
    }
    assert!(!directory.path().join("outside").exists());
    assert!(
        state
            .files
            .list("rename-client".into())
            .await
            .unwrap()
            .is_empty()
    );
    core.stop_core().await;
}
