use crate::test_runtime::Core as ContentServer;

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
async fn deletion_scan_failure_is_visible_after_cleanup_and_a_repeat_does_not_retry() {
    let directory = tempfile::tempdir().unwrap();
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
    assert!(
        first
            .err()
            .unwrap()
            .to_string()
            .contains("Rust core stopped")
    );
    assert!(!source.exists());
    assert_eq!(std::fs::read(&later).unwrap(), b"later");
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
