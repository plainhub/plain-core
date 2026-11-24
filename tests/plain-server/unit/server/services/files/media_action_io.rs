use plain_rs::features::enums::DataType;
use plain_rs::infrastructure::db::Db;
use std::path::Path;

#[cfg(all(feature = "media", not(any(target_os = "android", target_os = "ios"))))]
fn open_trash_fixture(root: &Path) -> crate::server::ContentServer {
    open_trash_fixture_with_catalog(root, false)
}

#[cfg(all(feature = "media", not(any(target_os = "android", target_os = "ios"))))]
fn open_trash_fixture_with_catalog(root: &Path, system_media_catalog: bool) -> crate::server::ContentServer {
    use plain_rs::features::prefs::{Prefs, default_path};
    use std::sync::Arc;

    let mut config = plain_rs::infrastructure::platform::startup::config();
    config.cache_dir = Some(root.join("cache").to_str().unwrap().into());
    config.filesystem_media_trash = true;
    config.system_media_catalog = system_media_catalog;
    let data = root.join("core");
    std::fs::create_dir_all(&data).unwrap();
    let prefs = Arc::new(Prefs::load(&default_path(&data)).unwrap());
    crate::server::ContentServer::start_core_with_config(
        &data,
        &plain_libs::crypto::gen_token(),
        prefs,
        config,
    )
    .unwrap()
}

#[cfg(all(feature = "media", not(any(target_os = "android", target_os = "ios"))))]
#[tokio::test]
async fn filesystem_trash_uses_shared_file_operations_with_an_active_system_catalog() {
    use plain_graphql::Request;
    use plain_libs::serde_json::{Value, json};

    for (kind, items, count, extension, mime) in [
        ("IMAGE", "images", "imageCount", "png", "image/png"),
        ("VIDEO", "videos", "videoCount", "mp4", "video/mp4"),
        ("AUDIO", "audios", "audioCount", "mp3", "audio/mpeg"),
        ("DOC", "docs", "docCount", "txt", "text/plain"),
    ] {
        let directory = plain_libs::temporary::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let core = open_trash_fixture_with_catalog(&root, true);
        let state = core.runtime_state();
        let photo = root.join(format!("sample.{extension}"));
        std::fs::write(&photo, b"owned media fixture").unwrap();
        let source = photo.to_str().unwrap().to_owned();
        let (generation, mut requests) = state.host.connect();
        let host = state.host.clone();
        let responder = tokio::spawn(async move {
            while let Some(request) = requests.recv().await {
                let result = match request["method"].as_str().unwrap() {
                    "systemCapabilities" => json!(["MEDIA_DELETE", "MEDIA_MOVE"]),
                    "systemAccessFacts" => {
                        json!({"granted":{"IMAGES":true,"VIDEOS":true,"AUDIO":true,"DOCS":true,"FILES":true}})
                    }
                    "systemMediaIdsFacts" => json!(["41"]),
                    "mediaProviderRows" => json!([{
                        "id":"41", "path":source, "trashed":false,
                        "mimeType":mime, "volumeRoot":null
                    }]),
                    "fileTaskScan" => json!(true),
                    _ => Value::Null,
                };
                host.reply(generation, json!({"id":request["id"],"result":result}))
                    .unwrap();
            }
        });
        let executor = core.native_graphql().unwrap();
        let moved = executor
            .execute(Request::new(format!(
                r#"mutation {{ trashMediaItems(type:{kind},query:"ids:41") {{ affectedCount }} }}"#
            )))
            .await;
        assert!(moved.errors.is_empty(), "{:?}", moved.errors);
        assert!(!photo.exists());
        let trash = executor.execute(Request::new(format!(
            r#"{{ {items}(query:"trash:true",offset:0,limit:10,sortBy:UPDATED_AT_DESC){{id path}} {count}(query:"trash:true") trashedFiles(query:"sample.{extension}",offset:0,limit:10){{path}} }}"#
        ))).await;
        assert!(trash.errors.is_empty(), "{:?}", trash.errors);
        let trash = trash.data.into_json().unwrap();
        assert_eq!(trash[count], 1);
        assert_eq!(trash[items][0]["path"], trash["trashedFiles"][0]["path"]);
        let id = trash[items][0]["id"].as_str().unwrap();
        let physical = Path::new(trash[items][0]["path"].as_str().unwrap());
        assert_eq!(std::fs::read(physical).unwrap(), b"owned media fixture");
        let restored = executor.execute(Request::new(format!(
            r#"mutation {{ restoreMediaItems(type:{kind},query:"ids:{id}") {{ affectedCount }} }}"#
        ))).await;
        assert!(restored.errors.is_empty(), "{:?}", restored.errors);
        assert_eq!(
            restored.data.into_json().unwrap()["restoreMediaItems"]["affectedCount"],
            1
        );
        assert_eq!(std::fs::read(photo).unwrap(), b"owned media fixture");
        assert!(!physical.exists());
        core.stop_core().await;
        responder.abort();
    }
}

#[cfg(all(feature = "media", not(any(target_os = "android", target_os = "ios"))))]
fn respond_to_media_host(
    host: std::sync::Arc<plain_rs::infrastructure::host::bridge::Host>,
) -> tokio::task::JoinHandle<()> {
    use plain_libs::serde_json::{Value, json};

    let (generation, mut requests) = host.connect();
    tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let result = match request["method"].as_str().unwrap() {
                "systemCapabilities" => json!(["MEDIA_DELETE", "MEDIA_MOVE"]),
                "systemAccessFacts" => json!({"granted":{
                    "IMAGES":true, "VIDEOS":true, "AUDIO":true, "DOCS":true
                }}),
                _ => Value::Null,
            };
            host.reply(generation, json!({"id":request["id"],"result":result}))
                .unwrap();
        }
    })
}

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
async fn desktop_media_trash_round_trips_through_native_and_browser_graphql() {
    use plain_libs::axum::{body, extract::State, http};
    use plain_libs::serde_json::{Value, json};
    use plain_rs::features::sessions::{SessionType, token_key};
    use plain_rs::infrastructure::transport::{http_request, http_response};

    let directory = plain_libs::temporary::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let core = open_trash_fixture(&root);
    let state = core.runtime_state();
    let responder = respond_to_media_host(state.host.clone());
    let native = core.native_graphql().unwrap();
    let mut browser =
        plain_rs::features::services::sessions::create(&state, "Browser fixture".into()).unwrap();
    browser.r#type = SessionType::Login;
    state.db.session_save(&browser).unwrap();
    let key = token_key(&browser.token).unwrap();
    let execute = async |local: bool, query: String| -> Value {
        if local {
            let response = native.execute(plain_graphql::Request::new(query)).await;
            return plain_libs::serde_json::to_value(response).unwrap();
        }
        let document = json!({"requestId":plain_rs::features::record_id::generate(),"query":query})
            .to_string();
        let request =
            http_request::encrypt_raw(&key, "POST", "/graphql", document.as_bytes()).unwrap();
        let mut headers = http::HeaderMap::new();
        headers.insert("c-id", browser.client_id.parse().unwrap());
        let response = crate::http::graphql::call(
            State(state.clone()),
            headers,
            body::Bytes::from(request.clone()),
        )
        .await;
        assert_eq!(response.status(), http::StatusCode::OK);
        let bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let payload = http_response::Context::new(&request)
            .decrypt(&key, &bytes)
            .unwrap();
        plain_libs::serde_json::from_slice(&payload).unwrap()
    };

    let protected = root.join("private.txt");
    std::fs::write(&protected, b"private fixture").unwrap();
    let id = seed_kind(&state.db, &protected, DataType::Doc);
    state
        .prefs
        .set("api_permissions", json!(["IMAGES", "VIDEOS", "AUDIO"]))
        .unwrap();
    for action in ["trashMediaItems", "restoreMediaItems"] {
        let denied = execute(
            false,
            format!("mutation {{ {action}(type:DOC,query:\"ids:{id}\") {{ affectedCount }} }}"),
        )
        .await;
        assert_eq!(denied["errors"][0]["message"], "NO_PERMISSION");
    }
    assert_eq!(std::fs::read(&protected).unwrap(), b"private fixture");
    state
        .prefs
        .set(
            "api_permissions",
            json!(["IMAGES", "VIDEOS", "AUDIO", "DOCS"]),
        )
        .unwrap();

    for local in [false, true] {
        let capabilities = execute(local, "{ app { capabilities } }".into()).await;
        assert!(
            capabilities["data"]["app"]["capabilities"]
                .as_array()
                .unwrap()
                .contains(&json!("MEDIA_TRASH")),
            "local={local}: {capabilities}"
        );
        for (kind, name, extension, field) in [
            (DataType::Image, "IMAGE", "jpg", "images"),
            (DataType::Video, "VIDEO", "mp4", "videos"),
            (DataType::Audio, "AUDIO", "mp3", "audios"),
            (DataType::Doc, "DOC", "txt", "docs"),
        ] {
            let original = root.join(format!("round-trip-{local}.{extension}"));
            let payload = format!("{name} fixture");
            std::fs::write(&original, &payload).unwrap();
            let id = seed_kind(&state.db, &original, kind);
            let mutate = |action| {
                format!(
                    "mutation {{ {action}(type:{name},query:\"ids:{id}\") {{ affectedCount }} }}"
                )
            };
            let list = |trash| {
                format!(
                    "{{ {field}(query:\"ids:{id} trash:{trash}\",offset:0,limit:10,sortBy:UPDATED_AT_DESC) {{ id path }} }}"
                )
            };

            let moved = execute(local, mutate("trashMediaItems")).await;
            assert_eq!(
                moved["data"]["trashMediaItems"]["affectedCount"], 1,
                "local={local}, type={name}: {moved}"
            );
            assert!(!original.exists());
            let trashed = execute(local, list(true)).await;
            let record = &trashed["data"][field][0];
            assert_eq!(record["id"], id, "{trashed}");
            let physical = Path::new(record["path"].as_str().unwrap());
            assert!(
                physical
                    .components()
                    .any(|part| part.as_os_str() == ".plain-trash")
            );
            assert_eq!(std::fs::read_to_string(physical).unwrap(), payload);
            assert_eq!(execute(local, list(false)).await["data"][field], json!([]));

            let restored = execute(local, mutate("restoreMediaItems")).await;
            assert_eq!(
                restored["data"]["restoreMediaItems"]["affectedCount"], 1,
                "local={local}, type={name}: {restored}"
            );
            assert_eq!(std::fs::read_to_string(&original).unwrap(), payload);
            assert!(!physical.exists());
            assert_eq!(execute(local, list(true)).await["data"][field], json!([]));
            assert_eq!(
                execute(local, list(false)).await["data"][field][0]["id"],
                id
            );
            assert_eq!(
                execute(local, mutate("restoreMediaItems")).await["data"]["restoreMediaItems"]["affectedCount"],
                0
            );

            if !local {
                assert_eq!(
                    execute(false, mutate("trashMediaItems")).await["data"]["trashMediaItems"]["affectedCount"],
                    1
                );
                assert_eq!(
                    execute(true, mutate("restoreMediaItems")).await["data"]["restoreMediaItems"]["affectedCount"],
                    1
                );
                assert_eq!(std::fs::read_to_string(&original).unwrap(), payload);
            }
        }
    }
    core.stop_core().await;
    responder.abort();
}

#[cfg(all(feature = "media", not(any(target_os = "android", target_os = "ios"))))]
#[tokio::test]
async fn desktop_image_trash_preserves_name_collisions_and_partial_batch_success() {
    use plain_graphql::{Request, Variables};
    use plain_libs::serde_json::json;

    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = open_trash_fixture(directory.path());
    let state = core.runtime_state();
    state.prefs.set("api_permissions", json!(["IMAGES"])).unwrap();
    let photo = directory.path().join("photo.jpg");
    std::fs::write(&photo, b"original image bytes").unwrap();
    let id = seed_kind(&state.db, &photo, DataType::Image);

    let responder = respond_to_media_host(state.host.clone());
    let selection = format!("ids:{id}");
    let mutate = |name| Request::new(format!(
        "mutation($query:String!) {{ {name}(type:IMAGE,query:$query) {{ affectedCount }} }}"
    )).variables(Variables::from_json(json!({"query":selection})));
    let native = core.native_graphql().unwrap();
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
        r#"{ trashedFiles(query:"photo.jpg",offset:0,limit:10) { name path } }"#
    )).await;
    assert!(file_trash.errors.is_empty(), "{:?}", file_trash.errors);
    let listed = file_trash.data.into_json().unwrap();
    assert_eq!(listed["trashedFiles"][0]["path"], physical.to_string_lossy().as_ref());
    assert_eq!(listed["trashedFiles"][0]["name"], "photo.jpg");
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
