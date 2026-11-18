use plain_rs::features::enums::DataType;
use plain_rs::infrastructure::db::Db;
use std::path::Path;

#[cfg(feature = "media")]
#[tokio::test]
async fn existing_media_delete_filters_trash_and_preserves_unselected_files() {
    let directory = plain_libs::temporary::tempdir().unwrap();
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
            plain_rs::infrastructure::db::MediaFileMetadata {
                album: String::new(),
                album_artist: String::new(),

                data_type: plain_rs::features::enums::DataType::Doc,
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
    let response = native.execute(plain_graphql::Request::new("mutation Delete($query: String!) { deleteMediaItems(type: DOC, query: $query) { affectedCount } }")
        .variables(plain_graphql::Variables::from_json(plain_libs::serde_json::json!({"query":format!("ids:{} trash:false",ids.join(","))})))).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["deleteMediaItems"]["affectedCount"], 1);
    assert!(!paths[0].exists());
    assert!(paths[1].exists());
    assert!(
        catalog
            .by_path(
                plain_rs::features::enums::DataType::Doc,
                paths[0].to_str().unwrap()
            )
            .unwrap()
            .is_none()
    );
    assert!(
        catalog
            .by_path(
                plain_rs::features::enums::DataType::Doc,
                paths[1].to_str().unwrap()
            )
            .unwrap()
            .is_some()
    );
    core.stop_core().await;
    let response = native
        .execute(plain_graphql::Request::new("{ __typename }"))
        .await;
    assert!(
        !response.errors.is_empty(),
        "stopped native executor must reject requests"
    );
}

#[cfg(feature = "media")]
#[tokio::test]
async fn image_delete_updates_index_before_any_parent_consumes_the_receipt() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let path = directory.path().join("photo.jpg");
    std::fs::write(&path, b"image").unwrap();
    let core = crate::server::ContentServer::open(&directory.path().join("core")).unwrap();
    let catalog = core.native_media_catalog().unwrap();
    let rows = catalog
        .record_files(&[plain_rs::infrastructure::db::MediaFileMetadata {
            album: String::new(),
            album_artist: String::new(),

            data_type: plain_rs::features::enums::DataType::Image,
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
    let state = core.runtime_state();
    let host = state.host.clone();
    let (generation, mut requests) = host.connect();
    let responder = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let reply = match request["method"].as_str().unwrap() {
                "systemCapabilities" => plain_libs::serde_json::json!(["MEDIA_DELETE", "MEDIA_MOVE"]),
                _ => plain_libs::serde_json::Value::Null,
            };
            host.reply(generation, plain_libs::serde_json::json!({"id":request["id"],"result":reply})).unwrap();
        }
    });
    let index = state.core.index.clone();
    let version = index.status().version;
    let response = core.native_graphql().unwrap().execute(plain_graphql::Request::new("mutation Delete($query: String!) { deleteMediaItems(type: IMAGE, query: $query) { affectedCount } }")
        .variables(plain_graphql::Variables::from_json(plain_libs::serde_json::json!({"query":format!("ids:{} trash:false",rows[0].id)})))).await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap()["deleteMediaItems"]["affectedCount"],
        1
    );
    assert!(!path.exists());
    assert_eq!(
        catalog
            .count(plain_rs::features::enums::DataType::Image)
            .unwrap(),
        0
    );
    assert_eq!(
        index.status().version,
        version + 1,
        "image index cleanup runs exactly once"
    );
    assert_eq!(index.status().indexed_images, 0);
    core.stop_core().await;
    responder.abort();
}

fn seed(db: &Db, path: &Path) -> String {
    seed_kind(db, path, DataType::Doc)
}

fn seed_kind(db: &Db, path: &Path, kind: DataType) -> String {
    db.record_media_files(&[plain_rs::infrastructure::db::MediaFileMetadata {
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
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state().core;
    let source = directory.path().join("note.txt");
    std::fs::write(&source, b"document").unwrap();
    let id = seed(&state.db, &source);
    let (_generation, mut requests) = state.host.connect();
    let mut events = state.events.subscribe();
    let response = core.native_graphql().unwrap().execute(plain_graphql::Request::new("mutation Delete($query: String!) { deleteMediaItems(type: DOC, query: $query) { affectedCount } }").variables(plain_graphql::Variables::from_json(plain_libs::serde_json::json!({"query":format!("ids:{id} trash:false")})))).await;
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
    use plain_rs::features::media::duration_cache::Duration;
    for (kind, name, media_type) in [
        (DataType::Audio, "AUDIO", "audio"),
        (DataType::Video, "VIDEO", "video"),
    ] {
        let directory = plain_libs::temporary::tempdir().unwrap();
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
        let response = core.native_graphql().unwrap().execute(plain_graphql::Request::new(format!(
            "mutation Delete($query: String!) {{ deleteMediaItems(type: {name}, query: $query) {{ affectedCount }} }}"
        )).variables(plain_graphql::Variables::from_json(plain_libs::serde_json::json!({"query":format!("ids:{good},{bad} trash:false")})))).await;
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

#[cfg(all(feature = "media", not(any(target_os = "android", target_os = "ios"))))]
#[tokio::test]
async fn local_desktop_image_trash_is_reversible_and_not_exposed_to_remote_clients() {
    use crate::graphql::executor::{RequestContext, execute_graphql};
    use plain_graphql::{Request, Variables};
    use plain_libs::serde_json::json;

    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state();
    state.prefs.set("api_permissions", json!(["IMAGES"])).unwrap();
    let photo = directory.path().join("photo.jpg");
    std::fs::write(&photo, b"original image bytes").unwrap();
    let id = seed_kind(&state.db, &photo, DataType::Image);
    let original_path = photo.canonicalize().unwrap();

    let host = state.host.clone();
    let (generation, mut requests) = host.connect();
    let responder = tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let reply = match request["method"].as_str().unwrap() {
                "systemCapabilities" => json!(["MEDIA_TRASH", "MEDIA_DELETE", "MEDIA_MOVE"]),
                "systemAccessFacts" => json!({"granted":{"IMAGES":true}}),
                _ => json!(null),
            };
            host.reply(generation, json!({"id":request["id"],"result":reply})).unwrap();
        }
    });
    let selection = format!("ids:{id}");
    let mutate = |name| Request::new(format!(
        "mutation($query:String!) {{ {name}(type:IMAGE,query:$query) {{ affectedCount }} }}"
    )).variables(Variables::from_json(json!({"query":selection})));
    let native = core.native_graphql().unwrap();
    let capabilities = "{ app { capabilities } }";
    let local = native.execute(Request::new(capabilities)).await;
    assert!(local.errors.is_empty(), "{:?}", local.errors);
    assert!(local.data.into_json().unwrap()["app"]["capabilities"].as_array().unwrap().contains(&json!("MEDIA_TRASH")));
    let remote = execute_graphql(&state.public, Request::new(capabilities),
        RequestContext::authenticated_remote(state.clone(),"fixture-remote".into())).await;
    assert!(remote.errors.is_empty(), "{:?}", remote.errors);
    assert!(!remote.data.into_json().unwrap()["app"]["capabilities"].as_array().unwrap().contains(&json!("MEDIA_TRASH")));
    let refused = execute_graphql(&state.public, mutate("trashMediaItems"),
        RequestContext::authenticated_remote(state.clone(),"fixture-remote".into())).await;
    assert_eq!(refused.errors[0].message, "unsupported capability MEDIA_TRASH");
    assert!(photo.is_file());

    let trashed = native.execute(mutate("trashMediaItems")).await;
    assert!(trashed.errors.is_empty(), "{:?}", trashed.errors);
    assert_eq!(trashed.data.into_json().unwrap()["trashMediaItems"]["affectedCount"], 1);
    assert!(!photo.exists());

    let list = |trash| Request::new(
        "query($query:String!) { images(query:$query,offset:0,limit:10,sortBy:UPDATED_AT_DESC) { id path } }"
    ).variables(Variables::from_json(json!({"query":format!("{selection} trash:{trash}")})));
    let trashed_rows = native.execute(list(true)).await;
    assert!(trashed_rows.errors.is_empty(), "{:?}", trashed_rows.errors);
    let data = trashed_rows.data.into_json().unwrap();
    assert_eq!(data["images"][0]["id"], id);
    let physical = Path::new(data["images"][0]["path"].as_str().unwrap());
    assert!(physical.components().any(|part| part.as_os_str()==".plain-trash"));
    assert_eq!(std::fs::read(physical).unwrap(), b"original image bytes");
    let file_trash = native.execute(Request::new(
        r#"{ trashedFiles(query:"photo.jpg",offset:0,limit:10,sortBy:DATE_DESC) { id originalPath trashedPath } }"#
    )).await;
    assert!(file_trash.errors.is_empty(), "{:?}", file_trash.errors);
    let listed = file_trash.data.into_json().unwrap();
    assert_eq!(listed["trashedFiles"][0]["id"], id);
    assert_eq!(listed["trashedFiles"][0]["trashedPath"], physical.to_string_lossy().as_ref());
    assert_eq!(listed["trashedFiles"][0]["originalPath"], original_path.to_string_lossy().as_ref());
    let active = native.execute(list(false)).await;
    assert!(active.errors.is_empty(), "{:?}", active.errors);
    assert_eq!(active.data.into_json().unwrap()["images"], json!([]));

    std::fs::write(&photo, b"new occupant").unwrap();
    let restored = native.execute(mutate("restoreMediaItems")).await;
    assert!(restored.errors.is_empty(), "{:?}", restored.errors);
    assert_eq!(restored.data.into_json().unwrap()["restoreMediaItems"]["affectedCount"], 1);
    let active = native.execute(list(false)).await;
    assert!(active.errors.is_empty(), "{:?}", active.errors);
    let data = active.data.into_json().unwrap();
    assert_eq!(data["images"][0]["id"], id);
    let restored = Path::new(data["images"][0]["path"].as_str().unwrap());
    assert_ne!(restored, photo);
    assert_eq!(std::fs::read(restored).unwrap(), b"original image bytes");
    assert_eq!(std::fs::read(&photo).unwrap(), b"new occupant");
    assert!(!physical.exists());

    let delete_active = native.execute(mutate("deleteMediaItems")).await;
    assert!(delete_active.errors.is_empty(), "{:?}", delete_active.errors);
    assert_eq!(delete_active.data.into_json().unwrap()["deleteMediaItems"]["affectedCount"], 0);
    assert!(restored.is_file(), "no fallback to deleting a live image");

    let second = directory.path().join("second.jpg");
    std::fs::write(&second, b"second image").unwrap();
    let second_id = seed_kind(&state.db, &second, DataType::Image);
    let missing = directory.path().join("unavailable.jpg");
    let missing_id = seed_kind(&state.db, &missing, DataType::Image);
    let changed = native.execute(Request::new(
        "mutation($query:String!) { trashMediaItems(type:IMAGE,query:$query) { affectedCount } }"
    ).variables(Variables::from_json(json!({"query":format!("ids:{second_id},{missing_id}")})))).await;
    assert!(changed.errors[0].message.contains("1 media actions failed (1 completed)"));
    assert!(!second.exists());
    assert!(!missing.exists(), "failed selection must not create a file or trash entry");
    let restore_second = native.execute(Request::new(
        "mutation($query:String!) { restoreMediaItems(type:IMAGE,query:$query) { affectedCount } }"
    ).variables(Variables::from_json(json!({"query":format!("ids:{second_id}")})))).await;
    assert!(restore_second.errors.is_empty(), "{:?}", restore_second.errors);
    assert_eq!(std::fs::read(second).unwrap(), b"second image");
    assert!(restored.is_file());

    core.stop_core().await;
    responder.abort();
}
