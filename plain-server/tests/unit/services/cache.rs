use super::*;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn native_cache_accounting_and_clear_preserve_application_data_without_host_requests() {
    let directory = tempfile::tempdir().unwrap();
    let paths = plain_rs::data_paths::DataPaths::new(directory.path());
    let cache = CachePaths::new(&directory.path().join("cache"));
    paths.initialize_directories().unwrap();
    cache.initialize(directory.path()).unwrap();
    let persistent = [
        paths.database(),
        paths.prefs(),
        paths.note_images().join("note.png"),
        paths.audio().join("sound.mp3"),
        paths.tls_identity(),
        directory.path().join("ai_models/model.onnx"),
        directory.path().join("uploads/active/chunk"),
        directory.path().join("archives/active.zip"),
    ];
    for path in &persistent {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"keep").unwrap();
    }
    for (path, bytes) in [
        (cache.root().join("remote_files/a"), b"remote".as_slice()),
        (cache.root().join("scratch"), b"temporary".as_slice()),
        (cache.image_cache().join("image"), b"image".as_slice()),
        (cache.video_cache().join("video"), b"video".as_slice()),
    ] {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    let host = crate::host::bridge::Host::default();
    let (_generation, mut requests) = host.connect();
    assert_eq!(size(&cache).unwrap(), 25);
    clear(&cache, &host, None).await.unwrap();
    assert_eq!(size(&cache).unwrap(), 0);
    assert!(cache.root().is_dir());
    for path in persistent {
        assert_eq!(std::fs::read(path).unwrap(), b"keep");
    }
    assert!(requests.try_recv().is_err());
    clear(&cache, &host, None).await.unwrap();
}

#[test]
fn portable_file_cleanup_leaves_sdk_indexes_to_their_owner() {
    let directory = tempfile::tempdir().unwrap();
    let cache = CachePaths::new(&directory.path().join("cache"));
    for path in [cache.image_cache(), cache.video_cache()] {
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("journal"), b"index").unwrap();
    }
    std::fs::write(cache.root().join("scratch"), b"remove").unwrap();
    clear_files(&cache, None).unwrap();
    assert_eq!(size(&cache).unwrap(), 10);
    assert!(!cache.root().join("scratch").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn cache_links_never_count_or_delete_external_files() {
    let directory = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    let paths = plain_rs::data_paths::DataPaths::new(directory.path());
    let cache = CachePaths::new(&directory.path().join("cache"));
    paths.initialize_directories().unwrap();
    cache.initialize(directory.path()).unwrap();
    let external_file = external.path().join("keep");
    std::fs::write(&external_file, b"keep").unwrap();
    std::os::unix::fs::symlink(external.path(), cache.root().join("linked-dir")).unwrap();
    std::os::unix::fs::symlink(&external_file, cache.root().join("linked-file")).unwrap();
    assert_eq!(size(&cache).unwrap(), 0);
    clear(&cache, &crate::host::bridge::Host::default(), None)
        .await
        .unwrap();
    assert_eq!(std::fs::read(&external_file).unwrap(), b"keep");
    assert_eq!(std::fs::read_dir(cache.root()).unwrap().count(), 0);
    std::fs::remove_dir(cache.root()).unwrap();
    std::os::unix::fs::symlink(external.path(), cache.root()).unwrap();
    assert!(size(&cache).is_err());
    assert!(
        clear(&cache, &crate::host::bridge::Host::default(), None)
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(&external_file).unwrap(), b"keep");
}

#[test]
fn absent_cache_is_empty_and_existing_invalid_roots_fail() {
    let directory = tempfile::tempdir().unwrap();
    let cache = CachePaths::new(&directory.path().join("cache"));
    assert_eq!(size(&cache).unwrap(), 0);
    clear_files(&cache, None).unwrap();
    std::fs::write(cache.root(), b"invalid").unwrap();
    assert!(size(&cache).is_err());
    assert!(clear_files(&cache, None).is_err());
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn cache_settings_execute_the_main_contract_without_platform_operations() {
    let directory = tempfile::tempdir().unwrap();
    let external_cache = tempfile::tempdir().unwrap();
    let server = crate::assembly::server::ContentServer::open_with_cache(
        directory.path(),
        external_cache.path(),
    )
    .unwrap();
    let native = server.native_graphql().unwrap();
    let mut events = server.runtime_state().events.subscribe();
    let cache = server.cache_paths().clone();
    std::fs::write(cache.root().join("scratch"), b"cached bytes").unwrap();
    let (_generation, mut requests) =
        crate::test_support::HostRequests::connect(&server.runtime_state().host);
    let response = native
        .execute(async_graphql::Request::new("{appCacheSize}"))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["appCacheSize"], 12);
    let response = native
        .execute(async_graphql::Request::new("mutation{clearAppCache}"))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(response.data.into_json().unwrap()["clearAppCache"], true);
    assert_eq!(size(&cache).unwrap(), 0);
    assert!(requests.try_recv().is_err());
    while let Ok(event) = events.try_recv() {
        assert_ne!(event.event_type, "CONTENT_CHANGED");
    }
    let state = server.runtime_state();
    let remote = crate::graphql::executor::execute_graphql(
        &state.public,
        async_graphql::Request::new("{appCacheSize}"),
        crate::graphql::executor::RequestContext::remote(Some(state.clone())),
    )
    .await;
    assert!(remote.errors.is_empty(), "{:?}", remote.errors);
    assert_eq!(remote.data.into_json().unwrap()["appCacheSize"], 0);
    server.shutdown().await;
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn simultaneous_cache_clears_are_idempotent() {
    let directory = tempfile::tempdir().unwrap();
    let cache = CachePaths::new(&directory.path().join("cache"));
    for path in [
        cache.image_cache(),
        cache.video_cache(),
        cache.root().join("remote_files"),
    ] {
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("cached"), b"data").unwrap();
    }
    let host = crate::host::bridge::Host::default();
    let (first, second) = tokio::join!(clear(&cache, &host, None), clear(&cache, &host, None));
    first.unwrap();
    second.unwrap();
    assert_eq!(size(&cache).unwrap(), 0);
}

#[test]
fn cache_byte_counts_preserve_64_bit_width_without_reading_contents() {
    let directory = tempfile::tempdir().unwrap();
    let paths = plain_rs::data_paths::DataPaths::new(directory.path());
    let cache = CachePaths::new(&directory.path().join("cache"));
    paths.initialize_directories().unwrap();
    cache.initialize(directory.path()).unwrap();
    let file = std::fs::File::create(cache.root().join("sparse")).unwrap();
    let bytes = 3 * 1024 * 1024 * 1024;
    file.set_len(bytes).unwrap();
    assert_eq!(size(&cache).unwrap(), bytes);
}

#[test]
fn portable_cache_clear_retains_only_the_active_tree_and_its_ancestors() {
    let dir = tempfile::tempdir().unwrap();
    let cache = CachePaths::new(&dir.path().join("cache"));
    let active = cache.root().join("retained").join("active");
    fs_create_cache_file(&active.join("assets/app.js"), b"keep");
    let unrelated = [
        cache.root().join("retained").join("inactive/index.html"),
        cache.root().join("retained").join("partial/index.html"),
        cache.root().join("scratch"),
    ];
    for path in &unrelated {
        fs_create_cache_file(path, b"remove");
    }
    clear_files(&cache, Some(&active)).unwrap();
    assert_eq!(
        std::fs::read(active.join("assets/app.js")).unwrap(),
        b"keep"
    );
    assert!(unrelated.iter().all(|path| !path.exists()));
    clear_files(&cache, Some(&active)).unwrap();
    assert_eq!(size(&cache).unwrap(), 4);
    clear_files(&cache, None).unwrap();
    assert_eq!(size(&cache).unwrap(), 0);
}

fn fs_create_cache_file(path: &std::path::Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

#[cfg(unix)]
#[test]
fn retained_asset_cleanup_unlinks_sibling_links_without_traversing_them() {
    let dir = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    let cache = CachePaths::new(&dir.path().join("cache"));
    let active = cache.root().join("retained").join("active");
    fs_create_cache_file(&active.join("index.html"), b"active");
    fs_create_cache_file(&external.path().join("index.html"), b"external");
    std::os::unix::fs::symlink(
        external.path(),
        cache.root().join("retained").join("linked"),
    )
    .unwrap();
    clear_files(&cache, Some(&active)).unwrap();
    assert_eq!(
        std::fs::read(external.path().join("index.html")).unwrap(),
        b"external"
    );
    assert_eq!(std::fs::read(active.join("index.html")).unwrap(), b"active");
    assert!(!cache.root().join("retained").join("linked").exists());
}
