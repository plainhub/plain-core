use super::*;

static NEXT_STAGING: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[test]
fn zip_virtual_paths_split_into_archive_and_prefix() {
    let dir = std::env::temp_dir().join(format!("plain_zip_path_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let archive = dir.join("pack.zip");
    std::fs::write(&archive, b"PK").unwrap();

    let (file, inner) = split_zip_path(&format!("{}!zip!/sub/deep", archive.display())).unwrap();
    assert_eq!(file, archive);
    assert_eq!(inner, "sub/deep");

    // A bare archive path is a plain file, exactly like `isZipPath` in the
    // Kotlin helper — only the explicit marker makes it a virtual path.
    assert!(split_zip_path(&archive.display().to_string()).is_none());
    let (file, inner) = split_zip_path(&format!("{}!zip!/", archive.display())).unwrap();
    assert_eq!(file, archive);
    assert_eq!(inner, "", "the archive root is an empty prefix");

    // A path that is not an existing archive is not a virtual path at all.
    assert!(split_zip_path(&format!("{}!zip!/x", dir.join("missing.zip").display())).is_none());
    assert!(split_zip_path(&dir.display().to_string()).is_none());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn download_names_are_percent_encoded() {
    assert_eq!(url_escape("plain.zip"), "plain.zip");
    assert_eq!(url_escape("报告 2024.zip"), "%E6%8A%A5%E5%91%8A%202024.zip");
    assert_eq!(url_escape("a\"b.zip"), "a%22b.zip");
}

#[test]
fn directory_walks_are_prefixed_and_recursive() {
    let dir = std::env::temp_dir().join(format!("plain_zip_walk_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("nested")).unwrap();
    std::fs::write(dir.join("top.txt"), b"a").unwrap();
    std::fs::write(dir.join("nested/inner.txt"), b"b").unwrap();
    let mut items = Vec::new();
    walk_directory(&dir, "bundle", &mut items).unwrap();
    let mut names: Vec<String> = items.iter().map(|(_, name)| name.clone()).collect();
    names.sort();
    assert_eq!(names, vec!["bundle/nested/inner.txt", "bundle/top.txt"]);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn zip_files_requests_default_every_field() {
    let request: ZipFilesRequest = serde_json::from_str("{}").unwrap();
    assert!(request.r#type.is_empty());
    assert!(request.query.is_empty());
    assert!(request.id.is_empty());
    assert!(request.name.is_empty());
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn encrypted_media_download_resolves_selection_and_preserves_each_basename_and_byte() {
    let directory = tempfile::tempdir().unwrap();
    let core =
        crate::assembly::server::ContentServer::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state();
    let (_generation, mut requests) = state.host.connect();
    let rows = (0..3)
        .map(|index| {
            let path = directory.path().join(format!("source-{index}.txt"));
            std::fs::write(&path, format!("bytes-{index}")).unwrap();
            plain_rs::db::MediaFileMetadata {
                data_type: DataType::Doc,
                path: path.to_string_lossy().into_owned(),
                title: format!("display title {index}"),
                artist: String::new(),
                album: String::new(),
                album_artist: String::new(),
                bucket_id: "fixture".into(),
                size_bytes: 7,
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
    let recorded = state.db.record_media_files(&rows).unwrap();
    let payload = json!({"type":"DOC","query":format!("ids:{},{}",recorded[0].id,recorded[2].id),"name":"selected.zip"}).to_string();
    let token = plain_rs::prefs::ensure_url_token(&state.prefs);
    let id = plain_rs::utils::base64::base64_encode_url_safe(
        &plain_rs::crypto::xchacha_encrypt(&token, payload.as_bytes()).unwrap(),
    );
    let response = files(State(state.clone()), Query(Params { id, sid: None })).await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    assert_eq!(archive.len(), 2);
    for index in [0, 2] {
        let mut bytes = String::new();
        archive
            .by_name(&format!("source-{index}.txt"))
            .unwrap()
            .read_to_string(&mut bytes)
            .unwrap();
        assert_eq!(bytes, format!("bytes-{index}"));
    }
    assert!(archive.by_name("source-1.txt").is_err());
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}

fn write_archive(path: &Path, files: &[(&str, &[u8])]) {
    let mut writer = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, body) in files {
        writer.start_file(*name, options).unwrap();
        writer.write_all(body).unwrap();
    }
    writer.finish().unwrap();
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "plain_zip_test_{}_{}_{}",
        name,
        std::process::id(),
        NEXT_STAGING.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn every_request_stages_into_its_own_directory() {
    let cache = tempfile::tempdir().unwrap();
    let first = Staging::create(cache.path()).unwrap();
    let second = Staging::create(cache.path()).unwrap();
    assert_eq!(first.directory.path().parent(), Some(cache.path()));
    assert_eq!(second.directory.path().parent(), Some(cache.path()));
    assert_ne!(
        first.directory.path(),
        second.directory.path(),
        "two requests must not share one staging directory"
    );

    std::fs::write(first.entry_path(0), b"first").unwrap();
    std::fs::write(second.entry_path(0), b"second").unwrap();

    assert_eq!(std::fs::read(first.entry_path(0)).unwrap(), b"first");
    assert_eq!(
        std::fs::read(second.entry_path(0)).unwrap(),
        b"second",
        "a concurrent zip must not overwrite another request's entry"
    );
}

#[test]
fn a_request_with_many_entries_keeps_them_apart() {
    let cache = tempfile::tempdir().unwrap();
    let staging = Staging::create(cache.path()).unwrap();
    std::fs::write(staging.entry_path(0), b"one").unwrap();
    std::fs::write(staging.entry_path(1), b"two").unwrap();

    assert_ne!(staging.entry_path(0), staging.entry_path(1));
    assert_eq!(std::fs::read(staging.entry_path(1)).unwrap(), b"two");
}

#[test]
fn staged_entries_disappear_when_the_request_ends() {
    let cache = tempfile::tempdir().unwrap();
    let staging = Staging::create(cache.path()).unwrap();
    std::fs::write(staging.entry_path(0), b"payload").unwrap();
    let directory = staging.directory.path().to_owned();
    assert!(directory.is_dir());

    drop(staging);

    assert!(
        !directory.exists(),
        "the staging directory outlived the request"
    );
}

#[test]
fn archive_entries_are_extracted_into_the_request_directory() {
    let dir = scratch("extract");
    let archive = dir.join("pack.zip");
    write_archive(&archive, &[("a.txt", b"alpha"), ("sub/b.txt", b"beta")]);
    let cache = tempfile::tempdir().unwrap();
    let staging = Staging::create(cache.path()).unwrap();
    let mut items = Vec::new();

    collect_archive_entries(&archive, "", &staging, &mut items).unwrap();

    assert_eq!(items.len(), 2);
    let mut names: Vec<String> = items.iter().map(|(_, name)| name.clone()).collect();
    names.sort();
    assert_eq!(names, vec!["a.txt", "sub/b.txt"]);
    for (path, _) in &items {
        assert_eq!(
            path.parent(),
            Some(staging.directory.path()),
            "entries must live inside this request's staging directory"
        );
        assert!(path.is_file());
    }
    assert_eq!(std::fs::read(&items[0].0).unwrap(), b"alpha");
    assert_eq!(std::fs::read(&items[1].0).unwrap(), b"beta");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_unreadable_archive_leaves_no_staged_files_behind() {
    let dir = scratch("broken");
    let archive = dir.join("broken.zip");
    std::fs::write(&archive, b"not a zip at all").unwrap();
    let cache = tempfile::tempdir().unwrap();
    let staging = Staging::create(cache.path()).unwrap();
    let directory = staging.directory.path().to_owned();
    let mut items = Vec::new();

    let failed = collect_archive_entries(&archive, "", &staging, &mut items).is_err();

    assert!(failed);
    assert!(items.is_empty());
    drop(staging);
    assert!(
        !directory.exists(),
        "a failed request must not leave staging files behind"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn shared_directory_download_preserves_empty_directories_and_excludes_outside_links() {
    for with_file in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let core =
            crate::assembly::server::ContentServer::open(&directory.path().join("core")).unwrap();
        let state = core.runtime_state();
        let (_generation, mut requests) = state.host.connect();
        state.prefs.set("background_service_enabled", true).unwrap();
        let folder = directory.path().join("photos");
        std::fs::create_dir_all(folder.join("empty")).unwrap();
        if with_file {
            std::fs::write(folder.join("a.txt"), "synthetic 中文").unwrap();
        }
        let outside = directory.path().join("outside.txt");
        std::fs::write(&outside, b"outside").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, folder.join("escape.txt")).unwrap();
        let shares = plain_rs::files::shares::Service::new(state.db.clone(), state.prefs.clone());
        let share = shares
            .create(
                "shared folder".into(),
                vec![folder.to_string_lossy().into_owned()],
                true,
                None,
            )
            .unwrap();
        let payload = serde_json::json!({"sharedId":share.id,"virtualPath":"photos"}).to_string();
        let key = plain_rs::utils::base64::base64_decode(&share.url_token);
        let id = plain_rs::utils::base64::base64_encode_url_safe(
            &plain_rs::crypto::xchacha_encrypt_raw(&key, payload.as_bytes()).unwrap(),
        );
        let response = dir(
            State(state.clone()),
            Query(Params {
                id,
                sid: Some(share.id),
            }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut names = archive.file_names().map(|name| name.unwrap().into_owned()).collect::<Vec<_>>();
        names.sort();
        assert_eq!(
            names,
            if with_file {
                vec!["a.txt", "empty/"]
            } else {
                vec!["empty/"]
            }
        );
        assert!(archive.by_name("empty/").unwrap().is_dir());
        if with_file {
            let mut text = String::new();
            archive
                .by_name("a.txt")
                .unwrap()
                .read_to_string(&mut text)
                .unwrap();
            assert_eq!(text, "synthetic 中文");
        }
        assert!(requests.try_recv().is_err());
        core.stop_core().await;
    }
}
