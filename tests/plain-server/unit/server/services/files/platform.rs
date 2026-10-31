#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn native_recent_files_use_the_owned_preferences_without_media_or_host_dispatch() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(&directory.path().join("core")).unwrap();
    let first = directory.path().join("first.txt");
    let last = directory.path().join("last.txt");
    std::fs::write(&first, b"first").unwrap();
    std::fs::write(&last, b"last").unwrap();
    let state = core.runtime_state().core;
    state
        .store
        .get()
        .unwrap()
        .set_json(
            "recent_files",
            vec![
                first.to_string_lossy().into_owned(),
                directory
                    .path()
                    .join("missing.txt")
                    .to_string_lossy()
                    .into_owned(),
                last.to_string_lossy().into_owned(),
            ],
        )
        .unwrap();
    let (_generation, mut requests) = state.host.connect();
    let response = core
        .native_graphql()
        .unwrap()
        .execute(async_graphql::Request::new(
            "{ recentFiles { name path size isDir } }",
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    let files = data["recentFiles"].as_array().unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0]["path"], first.to_string_lossy().as_ref());
    assert_eq!(files[1]["path"], last.to_string_lossy().as_ref());
    assert_eq!(files[0]["size"], 5);
    assert_eq!(files[0]["isDir"], false);
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}
