use super::*;
use plain_libs::serde_json::json;
use plain_rs::features::enums::DataType;
use std::io::Read;

#[test]
fn download_names_are_percent_encoded() {
    assert_eq!(crate::http::file_response::escape("plain.zip"), "plain.zip");
    assert_eq!(
        crate::http::file_response::escape("报告 2024.zip"),
        "%E6%8A%A5%E5%91%8A%202024.zip"
    );
    assert_eq!(crate::http::file_response::escape("a\"b.zip"), "a%22b.zip");
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn encrypted_media_download_preserves_all_selected_bytes_and_disambiguates_names() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = crate::server::ContentServer::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state();
    let (_generation, mut requests) = state.host.connect();
    let names = [
        "album-a/image.jpg",
        "album-b/image.jpg",
        "album-c/image (1).jpg",
        "album-a/README",
        "album-b/README",
        "album-a/not-selected.jpg",
    ];
    let rows = names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let path = directory.path().join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, format!("bytes-{index}")).unwrap();
            plain_rs::infrastructure::db::MediaFileMetadata {
                data_type: DataType::Image,
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
                updated_at: format!("2026-01-01T00:00:0{}.000Z", names.len() - index),
            }
        })
        .collect::<Vec<_>>();
    let recorded = state.db.record_media_files(&rows).unwrap();
    let ids = recorded[..5]
        .iter()
        .map(|row| row.id.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let payload =
        json!({"type":"IMAGE","query":format!("ids:{ids}"),"name":"selected.zip"}).to_string();
    let token = plain_libs::utils::base64::base64_encode(
        &plain_rs::features::sessions::token_key(&state.native_url_token().unwrap()).unwrap(),
    );
    let id = plain_rs::features::files::resource_link::encrypt(
        &plain_rs::features::files::resource_link::Scope::Session(
            state.native_client_id().unwrap(),
        ),
        plain_libs::aad::ResourcePurpose::ZipFiles,
        &plain_libs::utils::base64::base64_decode(&token),
        payload.as_bytes(),
    )
    .unwrap();
    let response = files(
        State(state.clone()),
        Query(Params { id }),
        Request::new(Body::empty()),
    )
    .await;
    let status = response.status();
    let bytes = plain_libs::axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let mut archive = plain_libs::zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    assert_eq!(archive.len(), 5);
    let entry_names = [
        "image.jpg",
        "image (2).jpg",
        "image (1).jpg",
        "README",
        "README (1)",
    ];
    for (index, name) in entry_names.into_iter().enumerate() {
        let expected = format!("bytes-{index}");
        let mut bytes = String::new();
        archive
            .by_name(name)
            .unwrap()
            .read_to_string(&mut bytes)
            .unwrap();
        assert_eq!(bytes, expected);
        assert_eq!(
            std::fs::read_to_string(directory.path().join(names[index])).unwrap(),
            expected
        );
    }
    assert!(archive.by_name("not-selected.jpg").is_err());
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}

#[tokio::test]
async fn shared_directory_download_preserves_empty_directories_and_excludes_outside_links() {
    for with_file in [false, true] {
        let directory = plain_libs::temporary::tempdir().unwrap();
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
        let shares =
            plain_rs::features::files::shares::Service::new(state.db.clone(), state.prefs.clone());
        let share = shares
            .create(plain_rs::features::files::shares::Input {
                name: "shared folder".into(),
                password: String::new(),
                data: (vec![folder.to_string_lossy().into_owned()])
                    .into_iter()
                    .map(|path: String| {
                        let path = std::path::PathBuf::from(path);
                        plain_rs::features::files::shares::SharePath {
                            virtual_path: format!(
                                "/{}",
                                path.file_name().unwrap().to_str().unwrap()
                            ),
                            real_path: path.to_str().unwrap().into(),
                            is_dir: path.is_dir(),
                        }
                    })
                    .collect(),
                read_only: true,
                expires_at: None,
            })
            .unwrap();
        let key = plain_libs::utils::base64::base64_decode(&share.url_token);
        let id = plain_rs::features::files::resource_link::seal(
            &plain_rs::features::files::resource_link::Scope::Share(share.id.clone()),
            &key,
            &plain_rs::features::files::resource_link::Resource {
                r#type: plain_rs::features::files::resource_link::ResourceType::SharedFile,
                id: "photos".into(),
            },
        )
        .unwrap();
        let response = dir(
            State(state.clone()),
            Query(Params { id }),
            Request::new(Body::empty()),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = plain_libs::axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let mut archive = plain_libs::zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
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
