use super::*;
use std::io::Read;
use serde_json::{Value, json};
use plain_rs::enums::DataType;





#[test]
fn download_names_are_percent_encoded() {
    assert_eq!(crate::http::file_response::escape("plain.zip"), "plain.zip");
    assert_eq!(crate::http::file_response::escape("报告 2024.zip"), "%E6%8A%A5%E5%91%8A%202024.zip");
    assert_eq!(crate::http::file_response::escape("a\"b.zip"), "a%22b.zip");
}



#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn encrypted_media_download_resolves_selection_and_preserves_each_basename_and_byte() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(&directory.path().join("core")).unwrap();
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
    let id = plain_rs::files::resource_link::encrypt(
        &plain_rs::files::resource_link::Scope::Global,
        "zip-files",
        &plain_rs::utils::base64::base64_decode(&token),
        payload.as_bytes(),
    )
    .unwrap();
    let response = files(State(state.clone()), Query(Params { id })).await;
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

#[tokio::test]
async fn shared_directory_download_preserves_empty_directories_and_excludes_outside_links() {
    for with_file in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let core = crate::server::ContentServer::open(&directory.path().join("core")).unwrap();
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
        let key = plain_rs::utils::base64::base64_decode(&share.url_token);
        let id = plain_rs::files::resource_link::seal(
            &plain_rs::files::resource_link::Scope::Share(share.id.clone()),
            &key,
            &plain_rs::files::resource_link::Resource {
                r#type: plain_rs::files::resource_link::ResourceType::SharedFile,
                id: "photos".into(),
            },
        )
        .unwrap();
        let response = dir(State(state.clone()), Query(Params { id })).await;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut names = archive
            .file_names()
            .map(|name| name.unwrap().into_owned())
            .collect::<Vec<_>>();
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
