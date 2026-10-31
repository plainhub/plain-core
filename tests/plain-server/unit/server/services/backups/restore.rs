use plain_rs::services::backups::test_fixture::archive;

#[tokio::test]
async fn native_restore_stages_without_overwriting_the_running_database() {
    let directory = tempfile::tempdir().unwrap();
    let archive_path = directory.path().join("selected-backup.zip");
    std::fs::write(&archive_path, archive()).unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state().core;
    state.db.with_conn(|conn| {
        conn.execute_batch(
            "CREATE TABLE live_fixture (value TEXT); INSERT INTO live_fixture VALUES ('live');",
        )
        .unwrap()
    });
    assert!(
        core.native_management()
            .unwrap()
            .stage_app_restore("app://selected-backup.zip".into())
            .await
            .unwrap()
    );
    assert_eq!(
        state
            .db
            .with_conn(
                |conn| conn.query_row("SELECT value FROM live_fixture", [], |row| row
                    .get::<_, String>(0))
            )
            .unwrap(),
        "live"
    );
    assert!(
        plain_rs::data_paths::directory::DataDirectory::open(
            directory.path(),
            core.cache_paths().root()
        )
        .err()
        .unwrap()
        .contains("process restart")
    );
    drop(state);
    core.shutdown().await;
}
