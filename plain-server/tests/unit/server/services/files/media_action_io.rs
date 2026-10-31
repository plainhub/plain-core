use plain_rs::db::Db;
use plain_rs::enums::DataType;
use std::path::Path;

#[cfg(feature = "media")]
#[tokio::test]
async fn existing_media_delete_filters_trash_and_preserves_unselected_files() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(&directory.path().join("core")).unwrap();
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
        .core
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
    let core = crate::server::ContentServer::open(&directory.path().join("core")).unwrap();
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
    let index = core.runtime_state().core.index.clone();
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
#[tokio::test]
async fn native_main_delete_has_no_host_exchange_and_publishes_each_event_once() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state().core;
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

#[cfg(feature = "media")]
#[tokio::test]
async fn partial_main_media_actions_invalidate_only_completed_duration_entries() {
    use plain_rs::media::duration_cache::Duration;
    for (kind, name, media_type) in [
        (DataType::Audio, "AUDIO", "audio"),
        (DataType::Video, "VIDEO", "video"),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let core = crate::server::ContentServer::open(&directory.path().join("core")).unwrap();
        let state = core.runtime_state().core;
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
