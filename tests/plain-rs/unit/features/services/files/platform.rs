use super::*;
use plain_libs::serde_json::json;

#[test]
fn native_read_access_rechecks_real_targets_and_rejects_missing_or_relative_paths() {
    let temp = plain_libs::temporary::tempdir().unwrap();
    let file = temp.path().join("file");
    std::fs::write(&file, b"bytes").unwrap();
    assert!(authorize_native(&file, false).is_ok());
    assert!(authorize_native(temp.path(), false).is_ok());
    assert!(authorize_native(Path::new("relative"), false).is_err());
    assert!(authorize_native(&temp.path().join("missing"), false).is_err());
    #[cfg(unix)]
    {
        let link = temp.path().join("link");
        std::os::unix::fs::symlink(&file, &link).unwrap();
        assert!(authorize_native(&link, false).is_ok());
        std::fs::remove_file(file).unwrap();
        assert!(authorize_native(&link, false).is_err());
    }
}

#[tokio::test]
async fn sdk_file_access_without_a_system_media_catalog_records_portable_file_metadata() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let source = crate::test_runtime::public_directory();
    let path = source.path().join("document.txt");
    std::fs::write(&path, b"owned catalog document").unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let host = Arc::new(Host::default());
    let platform = FilePlatform::new(
        host.clone(),
        state.db.clone(),
        state.directory.clone(),
        state.cache.root().to_owned(),
        #[cfg(not(feature = "runtime-media"))]
        state.events.clone(),
        #[cfg(feature = "runtime-media")]
        state.media_scans.clone(),
    );
    platform.use_sdk_for_test();
    let (generation, mut requests) = host.connect();
    let responder = host.clone();
    let worker = tokio::spawn(async move {
        let mut scans = 0;
        while let Some(request) = requests.recv().await {
            assert_eq!(request["method"], "fileTaskScan");
            scans += 1;
            responder
                .reply(generation, json!({"id":request["id"],"result":true}))
                .unwrap();
        }
        scans
    });
    platform
        .scan(&[path.to_string_lossy().into_owned()])
        .await
        .unwrap();
    let count = crate::features::services::files::media_catalog::count(
        &state.db,
        crate::features::enums::DataType::Doc,
        "",
    )
    .unwrap();
    host.disconnect(generation);
    let host_scans = worker.await.unwrap();
    core.stop_core().await;
    assert_eq!(count, 1);
    assert_eq!(host_scans, 0);
}

async fn sdk_recents(
    config: crate::infrastructure::host::startup::StartupConfig,
    volumes: plain_libs::serde_json::Value,
    provider_rows: plain_libs::serde_json::Value,
) -> (Result<Vec<plain_libs::serde_json::Value>>, Vec<String>) {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let host = Arc::new(Host::with_startup_config(config).unwrap());
    let platform = FilePlatform::new(
        host.clone(),
        state.db.clone(),
        directory.path().to_path_buf(),
        state.cache.root().to_owned(),
        #[cfg(not(feature = "runtime-media"))]
        state.events.clone(),
        #[cfg(feature = "runtime-media")]
        state.media_scans.clone(),
    );
    platform.use_sdk_for_test();
    let (generation, mut requests) = host.connect();
    let responder = host.clone();
    let worker = tokio::spawn(async move {
        let mut calls = Vec::new();
        while let Some(request) = requests.recv().await {
            let method = request["method"].as_str().unwrap();
            calls.push(method.to_owned());
            let result = match method {
                "systemMountFacts" => volumes.clone(),
                "systemFileIndexRows" => {
                    assert_eq!(request["params"]["limit"], 100);
                    assert_eq!(request["params"]["offset"], 0);
                    assert_eq!(request["params"]["sortBy"], "UPDATED_AT_DESC");
                    assert_eq!(
                        request["params"]["plan"]["clauses"],
                        json!(["mime_type != ?"])
                    );
                    assert_eq!(
                        request["params"]["plan"]["args"],
                        json!(["vnd.android.document/directory"])
                    );
                    provider_rows.clone()
                }
                other => panic!("unexpected recent file method {other}"),
            };
            responder
                .reply(generation, json!({"id":request["id"],"result":result}))
                .unwrap();
        }
        calls
    });
    let result = platform.recent_files().await;
    host.disconnect(generation);
    let calls = worker.await.unwrap();
    core.stop_core().await;
    (result, calls)
}

#[tokio::test]
async fn sdk_catalog_reuses_startup_metadata_and_queries_only_provider_rows() {
    let rows = json!([{"name":"catalog.txt","path":"/catalog.txt","size":42}]);
    let (result, calls) = sdk_recents(
        crate::infrastructure::host::startup::StartupConfig {
            recent_files_catalog: true,
            ..crate::infrastructure::platform::startup::config()
        },
        json!([]),
        rows.clone(),
    )
    .await;
    assert_eq!(result.unwrap(), rows.as_array().unwrap().clone());
    assert_eq!(calls, ["systemFileIndexRows"]);
}

#[tokio::test]
async fn sdk_filesystem_scan_runs_in_rust_without_provider_or_per_file_host_calls() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    std::fs::write(dir.path().join("file.txt"), b"sample").unwrap();
    let (result, calls) = sdk_recents(
        crate::infrastructure::host::startup::StartupConfig {
            recent_files_catalog: false,
            ..crate::infrastructure::platform::startup::config()
        },
        json!([{"driveType":"INTERNAL_STORAGE","path":dir.path()}]),
        json!([]),
    )
    .await;
    let files = result.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["name"], "file.txt");
    assert_eq!(files[0]["size"], 6);
    assert!(files[0]["createdAt"].is_null());
    assert!(files[0]["mediaId"].is_null());
    assert_eq!(calls, ["systemMountFacts"]);
}

#[tokio::test]
async fn invalid_recent_source_is_rejected_without_guessing_or_provider_fallback() {
    for volumes in [
        json!([]),
        json!([{"driveType":"INTERNAL_STORAGE","path":"relative"}]),
    ] {
        let (result, calls) = sdk_recents(
            crate::infrastructure::platform::startup::config(),
            volumes,
            json!([]),
        )
        .await;
        assert!(result.is_err());
        assert_eq!(calls, ["systemMountFacts"]);
    }
}

#[tokio::test]
async fn sdk_without_a_catalog_requires_a_primary_storage_directory() {
    let (result, calls) = sdk_recents(
        crate::infrastructure::platform::startup::config(),
        json!([]),
        json!([{ "name":"must not request provider rows" }]),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(calls, ["systemMountFacts"]);
}

#[tokio::test]
async fn thumbnail_cache_opens_the_file_without_sdk_requests_and_rechecks_access() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let state = core.runtime_state();
    state.file_platform.use_sdk_for_test();
    let (_, mut requests) = state.host.connect();
    let path = directory.path().join("image");
    std::fs::write(&path, []).unwrap();
    for _ in 0..2 {
        let cache = state
            .file_platform
            .thumbnail_cache(path.to_str().unwrap())
            .unwrap();
        assert_eq!(cache, state.cache.root().to_owned());
    }
    std::fs::remove_file(&path).unwrap();
    assert!(
        state
            .file_platform
            .thumbnail_cache(path.to_str().unwrap())
            .is_err()
    );
    std::fs::create_dir(&path).unwrap();
    assert!(
        state
            .file_platform
            .thumbnail_cache(path.to_str().unwrap())
            .is_err()
    );
    assert!(
        state
            .file_platform
            .thumbnail_cache("content://images/1")
            .is_err()
    );
    assert!(state.file_platform.thumbnail_cache("relative").is_err());
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}

#[cfg(unix)]
#[tokio::test]
async fn thumbnail_cache_rechecks_current_os_read_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let state = core.runtime_state();
    state.file_platform.use_sdk_for_test();
    let (_, mut requests) = state.host.connect();
    let path = directory.path().join("image");
    std::fs::write(&path, []).unwrap();
    state
        .file_platform
        .thumbnail_cache(path.to_str().unwrap())
        .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0)).unwrap();
    let can_open = std::fs::File::open(&path).is_ok();
    assert_eq!(
        state
            .file_platform
            .thumbnail_cache(path.to_str().unwrap())
            .is_ok(),
        can_open
    );
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    state
        .file_platform
        .thumbnail_cache(path.to_str().unwrap())
        .unwrap();
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}

#[tokio::test]
async fn metadata_authorization_batches_sdk_paths_without_requiring_existing_files() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state();
    state.file_platform.use_sdk_for_test();
    let (generation, mut requests) = state.host.connect();
    assert!(
        state
            .file_platform
            .authorize_metadata_paths(&["relative".into()])
            .await
            .is_err()
    );
    assert!(requests.try_recv().is_err());
    let paths = vec![
        directory
            .path()
            .join("missing-a")
            .to_string_lossy()
            .into_owned(),
        directory
            .path()
            .join("missing-b")
            .to_string_lossy()
            .into_owned(),
    ];
    let host = state.host.clone();
    let worker = tokio::spawn(async move {
        let request = requests.recv().await.unwrap();
        assert_eq!(request["method"], "systemAccessFacts");
        assert_eq!(request["params"], json!({"features":["FILES"]}));
        host.reply(
            generation,
            json!({"id":request["id"], "result":{"granted":{"FILES":true}}}),
        )
        .unwrap();
        requests
    });
    state
        .file_platform
        .authorize_metadata_paths(&paths)
        .await
        .unwrap();
    let mut requests = worker.await.unwrap();
    assert!(requests.try_recv().is_err());
    state.host.disconnect(generation);
    core.stop_core().await;
}

#[tokio::test]
async fn sdk_private_paths_skip_host_and_external_task_batches_sample_one_live_grant() {
    let temp = plain_libs::temporary::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(&temp.path().join("core")).unwrap();
    let state = core.runtime_state();
    state.file_platform.use_sdk_for_test();
    let (generation, mut requests) = state.host.connect();
    let private = vec![
        state
            .directory
            .join("new/file")
            .to_string_lossy()
            .into_owned(),
        state
            .cache
            .root()
            .join("new/file")
            .to_string_lossy()
            .into_owned(),
    ];
    state
        .file_platform
        .authorize_paths(&private, true)
        .await
        .unwrap();
    state.file_platform.scan(&private).await.unwrap();
    assert!(
        state
            .file_platform
            .identities(&private)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(requests.try_recv().is_err());
    assert!(
        state
            .file_platform
            .authorize_paths(&["/".into()], true)
            .await
            .is_err()
    );
    assert!(requests.try_recv().is_err());
    let host = state.host.clone();
    let worker = tokio::spawn(async move {
        let mut calls = 0;
        for granted in [true, false] {
            let request = requests.recv().await.unwrap();
            assert_eq!(request["method"], "systemAccessFacts");
            assert_eq!(request["params"], json!({"features":["FILES"]}));
            calls += 1;
            host.reply(
                generation,
                json!({"id":request["id"],"result":{"granted":{"FILES":granted}}}),
            )
            .unwrap();
        }
        (calls, requests)
    });
    let ops = (0..64)
        .map(|index| FileTaskOp {
            src: temp
                .path()
                .join(format!("source-{index}"))
                .to_string_lossy()
                .into_owned(),
            dst: temp
                .path()
                .join(format!("target-{index}"))
                .to_string_lossy()
                .into_owned(),
            overwrite: false,
        })
        .collect::<Vec<_>>();
    state
        .file_platform
        .authorize_task(FileTaskType::Move, &ops)
        .await
        .unwrap();
    assert!(
        state
            .file_platform
            .authorize_task(FileTaskType::Copy, &ops)
            .await
            .is_err()
    );
    let (calls, mut requests) = worker.await.unwrap();
    assert_eq!(calls, 2);
    assert!(requests.try_recv().is_err());
    state.host.disconnect(generation);
    core.stop_core().await;
}

#[tokio::test]
async fn automatic_indexing_excludes_private_hidden_nomedia_and_app_storage_paths() {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    let directory = plain_libs::temporary::Builder::new()
        .prefix("external-index-")
        .tempdir_in(home)
        .unwrap();
    let core = crate::test_runtime::Core::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state();
    let public = directory.path().join("public");
    std::fs::create_dir_all(&public).unwrap();
    for name in ["image.png", "song.wav", "clip.mp4", "document.txt"] {
        let path = public.join(name);
        std::fs::write(&path, b"fixture").unwrap();
        assert!(
            state.file_platform.indexable_type(&path).is_some(),
            "{name}"
        );
    }
    for path in [
        state.directory.join("private.png"),
        state.cache.root().join("cache.png"),
        public.join(".pending-1-image.png"),
        public.join(".hidden/image.png"),
        public.join("Android/data/app/image.png"),
        public.join("Android/obb/app/image.png"),
        public.join("archive.zip"),
    ] {
        assert!(
            state.file_platform.indexable_type(&path).is_none(),
            "{}",
            path.display()
        );
    }
    std::fs::write(public.join(".nomedia"), b"").unwrap();
    assert!(
        state
            .file_platform
            .indexable_type(&public.join("image.png"))
            .is_none()
    );
    assert!(state.file_platform.watchable(&public));
    assert!(
        !state
            .file_platform
            .watchable(Path::new("relative/image.png"))
    );
    assert!(!state.file_platform.watchable(&public.join("../image.png")));
    core.stop_core().await;
}

#[tokio::test]
async fn media_resource_lookup_uses_zero_sql_and_tracks_committed_changes() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let id=state.db.with_conn(|connection|{
        connection.execute("INSERT INTO volumes(id,identity,path,is_available) VALUES('media-lookup','media-lookup','/media-lookup',1)",[]).unwrap();
        let volume: String = connection.query_row("SELECT id FROM volumes ORDER BY rowid DESC LIMIT 1", [], |row| row.get(0)).unwrap();
        connection.execute("WITH RECURSIVE numbers(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM numbers WHERE n<2000) INSERT INTO files(id,volume_id,path,file_name,kind,updated_at) SELECT printf('file-%06d',n),?1,'other-'||n,'other-'||n,'FILE','2026-01-01' FROM numbers",[&volume]).unwrap();
        connection.execute("INSERT INTO files(id,volume_id,path,file_name,kind,updated_at) VALUES('file-'||'photo.jpg',?1,'photo.jpg','photo.jpg','IMAGE','2026-01-01')",[&volume]).unwrap();
        "file-photo.jpg".to_owned()
    });
    let steps = Arc::new(AtomicUsize::new(0));
    let observed = steps.clone();
    state.db.with_conn(|connection| {
        connection.progress_handler(
            1,
            Some(move || {
                observed.fetch_add(1, Ordering::Relaxed);
                false
            }),
        )
    });
    let file = state.file_platform.resource_media_file(&id).await.unwrap();
    state
        .db
        .with_conn(|connection| connection.progress_handler(0, None::<fn() -> bool>));
    assert_eq!(file.path, "/media-lookup/photo.jpg");
    assert_eq!(file.name, "photo.jpg");
    assert!(
        steps.load(Ordering::Relaxed) == 0,
        "single media lookup executed {} VM steps",
        steps.load(Ordering::Relaxed)
    );
    for invalid in [
        format!("0{id}"),
        format!("+{id}"),
        format!("{id}.0"),
        format!("{id} OR 1=1"),
        "media-uuid".into(),
    ] {
        assert!(
            state
                .file_platform
                .resource_media_file(&invalid)
                .await
                .is_err()
        );
    }
    let change = |sql: &str| state.db.with_conn(|c| c.execute_batch(sql)).unwrap();
    change("UPDATE files SET path='renamed.jpg' WHERE id='file-photo.jpg'");
    assert_eq!(
        state
            .file_platform
            .resource_media_file(&id)
            .await
            .unwrap()
            .path,
        "/media-lookup/renamed.jpg"
    );
    change("BEGIN; DELETE FROM files WHERE id='file-photo.jpg'; ROLLBACK");
    assert!(state.file_platform.resource_media_file(&id).await.is_ok());
    change("UPDATE volumes SET is_available=0 WHERE id='media-lookup'");
    assert!(state.file_platform.resource_media_file(&id).await.is_err());
    change("UPDATE volumes SET path='/remounted',is_available=1 WHERE id='media-lookup'");
    assert_eq!(
        state
            .file_platform
            .resource_media_file(&id)
            .await
            .unwrap()
            .path,
        "/remounted/renamed.jpg"
    );
    change(
        "INSERT OR REPLACE INTO files(id,volume_id,path,file_name,kind,updated_at) VALUES('replacement','media-lookup','renamed.jpg','renamed.jpg','IMAGE','2026')",
    );
    assert!(state.file_platform.resource_media_file(&id).await.is_err());
    change("UPDATE files SET trashed_at='2026',original_path='renamed.jpg' WHERE id='replacement'");
    assert!(
        state
            .file_platform
            .resource_media_file("replacement")
            .await
            .is_err()
    );
    change("UPDATE files SET trashed_at=NULL,original_path=NULL WHERE id='replacement'");
    assert!(
        state
            .file_platform
            .resource_media_file("replacement")
            .await
            .is_ok()
    );
    change("UPDATE files SET missing=1 WHERE id='replacement'");
    assert!(
        state
            .file_platform
            .resource_media_file("replacement")
            .await
            .is_err()
    );
    core.stop_core().await;
}

#[tokio::test]
async fn identity_receipts_exclude_unavailable_volume_aliases() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let state = core.runtime_state();
    state.db.with_conn(|connection| connection.execute_batch(
        "INSERT INTO volumes(id,identity,path,is_available) VALUES('old-volume','old-volume','/media-receipt',0),('current-volume','current-volume','/media-receipt',1);
         INSERT INTO files(id,volume_id,path,file_name,kind,updated_at) VALUES('old-file','old-volume','photo.jpg','photo.jpg','IMAGE','2026'),('current-file','current-volume','photo.jpg','photo.jpg','IMAGE','2026');"
    )).unwrap();
    let paths = vec!["/media-receipt/photo.jpg".to_owned()];
    let rows = state.file_platform.identities(&paths).await.unwrap();
    crate::features::services::files::file_task_media::validate(&rows, &paths).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].file_id, "current-file");
    state
        .db
        .with_conn(|connection| {
            connection.execute(
                "UPDATE volumes SET is_available=0 WHERE id='current-volume'",
                [],
            )
        })
        .unwrap();
    assert!(
        state
            .file_platform
            .identities(&paths)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        state
            .db
            .with_conn(|connection| connection.query_row(
                "SELECT count(*) FROM files WHERE volume_id IN ('old-volume','current-volume')",
                [],
                |row| row.get::<_, i64>(0)
            ))
            .unwrap(),
        2
    );
    core.stop_core().await;
}
