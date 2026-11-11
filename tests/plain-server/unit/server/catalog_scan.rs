fn catalog_test_core(directory: &std::path::Path) -> crate::server::ContentServer {
    let core = crate::server::ContentServer::open(&directory.join("data")).unwrap();
    #[cfg(feature = "media")]
    core.runtime_state()
        .media_scans
        .set_defaults(plain_rs::features::files::index_scope::Defaults {
            excluded_directories: Vec::new(),
            excluded_names: Vec::new(),
        })
        .unwrap();
    core
}

#[tokio::test]
async fn catalog_is_created_from_core_and_stopped_handles_reject_access() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = catalog_test_core(directory.path());
    let catalog = core.native_media_catalog().unwrap();
    assert_eq!(
        catalog
            .count(plain_rs::features::enums::DataType::Image)
            .unwrap(),
        0
    );
    assert!(catalog.record_files(&[]).unwrap().is_empty());
    assert!(
        catalog
            .list(plain_rs::features::enums::DataType::Image, 10, 0)
            .unwrap()
            .is_empty()
    );
    core.stop_core().await;
    assert!(core.native_media_catalog().is_err());
    assert!(
        catalog
            .count(plain_rs::features::enums::DataType::Image)
            .is_err()
    );
    assert!(
        catalog
            .by_path(plain_rs::features::enums::DataType::Image, "/photo.jpg")
            .is_err()
    );
    assert!(
        catalog
            .list(plain_rs::features::enums::DataType::Image, 10, 0)
            .is_err()
    );
    assert!(catalog.record_files(&[]).is_err());
}

#[cfg(all(feature = "media", not(any(target_os = "android", target_os = "ios"))))]
#[tokio::test]
async fn native_file_metadata_reaches_catalog_without_any_host_request() {
    let directory = crate::test_support::public_directory();
    let path = directory.path().join("report.txt");
    std::fs::write(&path, "document").unwrap();
    let core = catalog_test_core(directory.path());
    let host = core.runtime_state().host.clone();
    let (_generation, mut requests) = crate::test_support::HostRequests::connect(&host);
    let catalog = core.native_media_catalog().unwrap();
    let indexed = catalog
        .scan_file(
            plain_rs::features::enums::DataType::Doc,
            path.to_str().unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(indexed.size_bytes, 8);
    assert_eq!(
        catalog
            .count_matching(plain_rs::features::enums::DataType::Doc, "ext:txt")
            .unwrap(),
        1
    );
    let rows = catalog
        .search(
            plain_rs::features::enums::DataType::Doc,
            "ext:txt",
            plain_rs::features::prefs::settings::FileSortBy::NameAsc,
            10,
            0,
        )
        .unwrap()
        .iter()
        .map(plain_rs::features::services::files::media_catalog_facts::row_facts)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(rows[0]["id"], indexed.id);
    assert_eq!(rows[0]["size"], 8);
    assert!(!rows[0].as_object().unwrap().contains_key("originalPath"));
    std::fs::remove_file(&path).unwrap();
    assert!(
        catalog
            .scan_file(
                plain_rs::features::enums::DataType::Doc,
                path.to_str().unwrap()
            )
            .await
            .is_err()
    );
    assert_eq!(
        catalog
            .count(plain_rs::features::enums::DataType::Doc)
            .unwrap(),
        1
    );
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
    assert!(
        catalog
            .scan_file(
                plain_rs::features::enums::DataType::Doc,
                path.to_str().unwrap()
            )
            .await
            .is_err()
    );
}

#[cfg(feature = "media")]
#[tokio::test]
async fn rescan_with_failed_probe_retains_current_known_metadata_without_host_round_trips() {
    let directory = crate::test_support::public_directory();
    let path = directory.path().join("damaged.mp3");
    std::fs::write(&path, b"unrecognized audio bytes").unwrap();
    let core = catalog_test_core(directory.path());
    let catalog = core.native_media_catalog().unwrap();
    let mut known = plain_rs::features::media::file_facts::read_file_metadata(
        &path,
        plain_rs::features::enums::DataType::Audio,
    )
    .unwrap();
    known.title = "Known title".into();
    known.artist = "Known artist".into();
    known.duration_ms = 1500;
    known.cover_image_id = Some("known-album".into());
    known.size_bytes = 999;
    let original = catalog.record_files(&[known]).unwrap().remove(0);
    let (_generation, mut requests) =
        crate::test_support::HostRequests::connect(&core.runtime_state().host);
    let updated = catalog
        .scan_file(
            plain_rs::features::enums::DataType::Audio,
            path.to_str().unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(updated.id, original.id);
    assert_eq!(updated.title, original.title);
    assert_eq!(updated.artist, original.artist);
    assert_eq!(updated.duration_ms, original.duration_ms);
    assert_eq!(updated.cover_image_id, original.cover_image_id);
    assert_eq!(
        updated.size_bytes,
        std::fs::metadata(path).unwrap().len() as i64
    );
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
    assert!(catalog.record_file_reads(&[]).is_err());
}

#[cfg(feature = "media")]
fn directory_scan_fixture() -> plain_libs::temporary::TempDir {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    plain_libs::temporary::Builder::new()
        .prefix("media-scan-")
        .tempdir_in(home)
        .unwrap()
}

#[cfg(feature = "media")]
#[tokio::test]
async fn directory_scan_uses_core_policy_real_walker_and_existing_catalog() {
    use plain_rs::features::services::files::media_scans::Status;
    let directory = directory_scan_fixture();
    let root = directory.path().join("library");
    let data = root.join("core");
    let cache = root.join("system-cache");
    let excluded = root.join("excluded");
    for folder in [
        &root,
        &cache,
        &excluded,
        &root.join("nested"),
        &root.join(".hidden"),
        &root.join("repository/.git"),
        &root.join(".plain-trash"),
        &root.join("trash"),
        &root.join("lost+found"),
        &root.join("marked"),
    ] {
        std::fs::create_dir_all(folder).unwrap();
    }
    for name in [
        "one.txt",
        "nested/two.txt",
        "photo.heic",
        "excluded/excluded.txt",
        ".hidden/hidden.txt",
        "repository/.git/internal.txt",
        ".plain-trash/deleted.txt",
        "trash/kept.txt",
        "lost+found/internal.txt",
        "marked/marked.txt",
    ] {
        std::fs::write(root.join(name), b"media facts").unwrap();
    }
    std::fs::write(cache.join("cache.txt"), b"cached facts").unwrap();
    std::fs::write(root.join("empty.txt"), b"").unwrap();
    std::fs::write(root.join("marked/.nomedia"), b"").unwrap();
    let config = directory.path().join("config.toml");
    std::fs::write(
        &config,
        format!("[media_scan]\nexcluded_dirs = {}\n", excluded.display()),
    )
    .unwrap();
    let core = crate::server::ContentServer::open_with_cache(&data, &cache).unwrap();
    std::fs::write(data.join("internal.txt"), b"internal").unwrap();
    let catalog = core.native_media_catalog().unwrap();
    assert_eq!(
        core.runtime_state()
            .media_scans
            .policy()
            .unwrap()
            .cache_directory,
        cache.canonicalize().unwrap()
    );
    core.runtime_state()
        .media_scans
        .set_defaults(plain_rs::features::files::index_scope::Defaults {
            excluded_directories: Vec::new(),
            excluded_names: vec!["lost+found".into()],
        })
        .unwrap();
    plain_rs::features::services::files::indexing::set_directories(
        &core.runtime_state().db,
        &[directory.path().to_string_lossy().into_owned()],
    )
    .unwrap();
    catalog.configure_scan_directories(Some(&config)).unwrap();
    let host = core.runtime_state().host.clone();
    let (generation, mut requests) = crate::test_support::HostRequests::connect(&host);
    let (done, mut finish) = tokio::sync::oneshot::channel::<()>();
    let authorize = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut finish => break,
                request=requests.recv() => {
                    let request=request.unwrap();
                    assert_eq!(request["method"],"systemAccessFacts");
                    assert_eq!(request["params"]["features"],serde_json::json!(["FILES"]));
                    host.reply(generation,serde_json::json!({"id":request["id"],"result":{"granted":{"FILES":true}}})).unwrap();
                },
            }
        }
    });
    let result = catalog
        .scan_directory(root.to_str().unwrap())
        .await
        .unwrap();
    done.send(()).unwrap();
    authorize.await.unwrap();
    assert_eq!(result.status, Status::Completed);
    assert_eq!(result.total_files, Some(6));
    assert_eq!(result.processed_files, 6);
    assert_eq!(
        catalog
            .count(plain_rs::features::enums::DataType::Doc)
            .unwrap(),
        4
    );
    assert_eq!(
        catalog
            .count(plain_rs::features::enums::DataType::Image)
            .unwrap(),
        1
    );
    assert!(
        catalog
            .by_path(
                plain_rs::features::enums::DataType::Doc,
                cache.join("cache.txt").to_str().unwrap()
            )
            .unwrap()
            .is_none()
    );
    core.stop_core().await;
}

#[cfg(feature = "media")]
#[tokio::test]
async fn rescanning_ts_files_keeps_only_transport_streams_in_video_search() {
    use plain_rs::features::enums::DataType;
    use plain_rs::features::prefs::settings::FileSortBy;
    use plain_rs::infrastructure::db::FileKind;

    let directory = directory_scan_fixture();
    let root = directory.path().join("library");
    std::fs::create_dir(&root).unwrap();
    let script = root.join("example.client.ts");
    std::fs::write(&script, b"export const example = true;\n").unwrap();
    let video = include_bytes!("../../../../plain-rs/testdata/video-h264.ts");
    for (stride, offset) in [(188, 0), (192, 4), (204, 0)] {
        let mut packets = vec![0; stride * (video.len() / 188)];
        for (index, packet) in video.chunks_exact(188).enumerate() {
            let start = offset + index * stride;
            packets[start..start + 188].copy_from_slice(packet);
        }
        std::fs::write(root.join(format!("example-{stride}.TS")), packets).unwrap();
    }
    std::fs::write(root.join("example-empty.ts"), b"").unwrap();
    std::fs::write(root.join("example-sync-bytes.ts"), "G".repeat(1024)).unwrap();
    let core = catalog_test_core(&directory.path().join("core"));
    let state = core.runtime_state();
    let catalog = core.native_media_catalog().unwrap();
    let previous = state
        .db
        .record_file_paths(&[script.to_str().unwrap().into()], Some(FileKind::Video))
        .unwrap()
        .remove(0);

    catalog
        .scan_directory(root.to_str().unwrap())
        .await
        .unwrap();
    let hits = catalog
        .search(DataType::Video, "example", FileSortBy::NameAsc, 10, 0)
        .unwrap();
    assert_eq!(
        hits.iter()
            .map(|row| std::path::Path::new(&row.path).file_name().unwrap().to_str().unwrap())
            .collect::<Vec<_>>(),
        ["example-188.TS", "example-192.TS", "example-204.TS"]
    );
    assert_eq!(
        catalog.count_matching(DataType::Video, "example").unwrap(),
        3
    );
    let script_row = state.db.file(&previous.id).unwrap().unwrap();
    assert_eq!(script_row.kind, FileKind::File);
    assert_ne!(script_row.mime_type.as_deref(), Some("video/mp2t"));
    assert!(!script_row.missing);
    core.stop_core().await;
}

#[cfg(feature = "media")]
#[tokio::test]
async fn completion_scan_uses_the_object_protocol_and_os_reads_without_host_reentry() {
    let directory = directory_scan_fixture();
    let core = catalog_test_core(&directory.path().join("core"));
    let catalog = core.native_media_catalog().unwrap();
    plain_rs::features::services::files::indexing::set_directories(
        &core.runtime_state().db,
        &[directory.path().to_string_lossy().into_owned()],
    )
    .unwrap();
    catalog.configure_scan_directories(None).unwrap();
    let path = directory.path().join("completed.txt");
    std::fs::write(&path, b"completed file").unwrap();
    assert!(!core.runtime_state().host.connected());
    assert!(
        catalog
            .scan_completed_files(serde_json::json!({"paths":[path]}))
            .await
            .unwrap()
    );
    assert_eq!(
        catalog
            .count(plain_rs::features::enums::DataType::Doc)
            .unwrap(),
        1
    );
    assert!(
        catalog
            .scan_completed_files(serde_json::json!([path]))
            .await
            .is_err()
    );
    assert!(
        catalog
            .scan_completed_files(serde_json::json!({"paths":[path],"extra":true}))
            .await
            .is_err()
    );
    assert!(
        catalog
            .scan_completed_files(serde_json::json!({"paths":[]}))
            .await
            .unwrap()
    );
    let opaque = directory.path().join("file.bin");
    std::fs::write(&opaque, b"opaque").unwrap();
    assert!(
        catalog
            .scan_completed_files(serde_json::json!({"paths":[opaque]}))
            .await
            .unwrap()
    );
    assert_eq!(
        catalog
            .count(plain_rs::features::enums::DataType::Doc)
            .unwrap(),
        1
    );
    core.stop_core().await;
    assert!(
        catalog
            .scan_completed_files(serde_json::json!({"paths":[]}))
            .await
            .is_err()
    );
}

#[cfg(feature = "media")]
#[tokio::test]
async fn completion_scan_reports_read_failures_instead_of_partial_success() {
    let directory = directory_scan_fixture();
    let core = catalog_test_core(&directory.path().join("core"));
    let catalog = core.native_media_catalog().unwrap();
    plain_rs::features::services::files::indexing::set_directories(
        &core.runtime_state().db,
        &[directory.path().to_string_lossy().into_owned()],
    )
    .unwrap();
    catalog.configure_scan_directories(None).unwrap();
    let path = directory.path().join("completed.txt");
    std::fs::write(&path, b"completed file").unwrap();
    let missing = directory.path().join("missing.txt");
    assert!(
        catalog
            .scan_completed_files(serde_json::json!({"paths":[path,missing]}))
            .await
            .is_err()
    );
    assert_eq!(
        catalog
            .count(plain_rs::features::enums::DataType::Doc)
            .unwrap(),
        0
    );
    assert!(
        catalog
            .scan_completed_files(serde_json::json!({"paths":["relative.txt"]}))
            .await
            .is_err()
    );
    core.stop_core().await;
}

#[tokio::test]
async fn source_directories_use_volume_configuration_and_reject_stopped_handles() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = catalog_test_core(directory.path());
    let catalog = core.native_media_catalog().unwrap();
    let clone = catalog.clone();
    let state = core.runtime_state();
    let source = directory.path().join("documents");
    std::fs::create_dir(&source).unwrap();
    let source =
        plain_rs::features::files::index_scope::normalized(&source.canonicalize().unwrap());
    plain_rs::features::services::files::indexing::set_directories(
        &state.db,
        &[source.clone(), source.clone()],
    )
    .unwrap();
    assert_eq!(clone.source_directories().unwrap(), vec![source]);
    let volume = state.db.volumes().unwrap().remove(0);
    assert_eq!(
        volume.index_mode,
        plain_rs::infrastructure::db::IndexMode::Allowlist
    );
    core.stop_core().await;
    assert!(catalog.source_directories().is_err());
    assert!(
        plain_rs::features::services::files::indexing::set(
            &state,
            &volume.id,
            plain_rs::infrastructure::db::IndexMode::Disabled,
            vec![],
            vec![]
        )
        .await
        .is_err()
    );
}

/// The watcher writes into the catalog, so it must not outlive the core
/// that owns it: after `stop_core` no filesystem event can reach the store.
#[cfg(feature = "media")]
#[tokio::test]
async fn the_watcher_stops_with_the_core_and_rejects_after_it() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = catalog_test_core(directory.path());
    let source = directory.path().join("media");
    std::fs::create_dir_all(&source).unwrap();
    let catalog = core.native_media_catalog().unwrap();
    plain_rs::features::services::files::indexing::set_directories(
        &core.runtime_state().db,
        &[source.to_string_lossy().into_owned()],
    )
    .unwrap();
    assert!(catalog.watch_source_directories().unwrap());
    plain_rs::features::services::files::indexing::set_directories(
        &core.runtime_state().db,
        &[directory.path().to_string_lossy().into_owned()],
    )
    .unwrap();
    catalog.configure_scan_directories(None).unwrap();
    assert!(catalog.watch_source_directories().unwrap());
    assert!(
        core.runtime_state().media_watch.lock().unwrap().is_some(),
        "a configured source directory must be watched"
    );
    // Re-watching replaces rather than stacking a second one.
    assert!(catalog.watch_source_directories().unwrap());

    core.stop_core().await;
    assert!(
        core.runtime_state().media_watch.lock().unwrap().is_none(),
        "the watcher has to be released with the core"
    );
    assert!(catalog.watch_source_directories().is_err());
    assert!(
        catalog
            .scan_completed_files(serde_json::json!({"paths":[]}))
            .await
            .is_err(),
        "a stopped core must refuse catalog writes"
    );
}

#[cfg(feature = "media")]
#[tokio::test]
async fn filesystem_watch_events_from_an_os_thread_update_the_shared_catalog() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let source = crate::test_support::public_directory();
    let core = catalog_test_core(directory.path());
    let catalog = core.native_media_catalog().unwrap();
    plain_rs::features::services::files::indexing::set_directories(
        &core.runtime_state().db,
        &[directory.path().to_string_lossy().into_owned()],
    )
    .unwrap();
    catalog.configure_scan_directories(None).unwrap();
    plain_rs::features::services::files::indexing::set_directories(
        &core.runtime_state().db,
        &[source.path().to_string_lossy().into_owned()],
    )
    .unwrap();
    catalog.configure_scan_directories(None).unwrap();
    catalog.watch_source_directories().unwrap();
    let path = source.path().join("photo.png");
    plain_libs::image::RgbImage::new(1, 1).save(&path).unwrap();
    let path = path.to_string_lossy().into_owned();
    let observed = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            if catalog
                .by_path(plain_rs::features::enums::DataType::Image, &path)
                .unwrap()
                .is_some()
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await;
    core.stop_core().await;
    assert!(
        observed.is_ok(),
        "OS watcher events must reach the owning Tokio runtime"
    );
}

#[cfg(feature = "media")]
#[tokio::test]
async fn nomedia_reclassification_preserves_file_identity_and_user_state() {
    use plain_rs::features::enums::DataType;
    use plain_rs::infrastructure::db::FileKind;
    let directory = directory_scan_fixture();
    let core = catalog_test_core(&directory.path().join("core"));
    let catalog = core.native_media_catalog().unwrap();
    plain_rs::features::services::files::indexing::set_directories(
        &core.runtime_state().db,
        &[directory.path().to_string_lossy().into_owned()],
    )
    .unwrap();
    catalog.configure_scan_directories(None).unwrap();
    let path = directory.path().join("song.wav");
    std::fs::write(&path, b"unrecognized audio bytes").unwrap();
    let scan = async || {
        catalog
            .scan_completed_files(serde_json::json!({"paths":[path]}))
            .await
            .unwrap()
    };
    scan().await;
    let original = catalog
        .by_path(DataType::Audio, path.to_str().unwrap())
        .unwrap()
        .unwrap();
    let id = &original.id;
    let db = core.runtime_state().db.clone();
    db.with_conn(|conn| -> rusqlite::Result<()> {
        conn.execute("UPDATE files SET favorited=1 WHERE id=?1", [id])?;
        conn.execute("INSERT INTO tags(id,name,type,created_at,updated_at) VALUES('nomedia-tag','Tag',1,'2026-01-01','2026-01-01')", [])?;
        conn.execute("INSERT INTO tag_relations(tag_id,target_key,type,created_at) VALUES('nomedia-tag',?1,?2,'2026-01-01')", rusqlite::params![original.id, DataType::Audio.kind()])?;
        Ok(())
    }).unwrap();
    let marker = directory.path().join(".nomedia");
    std::fs::write(&marker, b"").unwrap();
    let mut events = core.runtime_state().events.subscribe();
    scan().await;
    let event = events.try_recv().unwrap();
    assert_eq!(
        event.event_type,
        plain_rs::infrastructure::events::MEDIA_LIBRARY_UPDATED
    );
    assert!(
        plain_rs::features::services::files::catalog_events::includes(
            &event.payload,
            DataType::Audio
        )
    );
    assert!(
        plain_rs::features::services::files::catalog_events::includes(
            &event.payload,
            DataType::File
        )
    );
    assert_eq!(catalog.count(DataType::Audio).unwrap(), 0);
    let file = db.file(id).unwrap().unwrap();
    assert_eq!(file.kind, FileKind::File);
    assert!(file.favorited);
    std::fs::remove_file(marker).unwrap();
    scan().await;
    assert_eq!(catalog.count(DataType::Audio).unwrap(), 1);
    assert_eq!(
        catalog
            .by_path(DataType::Audio, path.to_str().unwrap())
            .unwrap()
            .unwrap()
            .id,
        original.id
    );
    assert!(db.file(id).unwrap().unwrap().favorited);
    assert_eq!(
        db.with_conn(|conn| conn.query_row(
            "SELECT count(*) FROM tag_relations WHERE tag_id='nomedia-tag' AND target_key=?1",
            [&original.id],
            |row| row.get::<_, i64>(0)
        ))
        .unwrap(),
        1
    );
    core.stop_core().await;
}
