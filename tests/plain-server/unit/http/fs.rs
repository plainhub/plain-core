use crate::server::ContentServer;
use crate::server::ServerState;
use plain_libs::axum::body::Body;
use plain_libs::axum::extract::Request;
use plain_libs::axum::http::{HeaderName, StatusCode, header};
use plain_libs::axum::response::Response;
use plain_libs::protocols::http_client;

fn test_state() -> (
    plain_libs::temporary::TempDir,
    ContentServer,
    ServerState,
    Vec<u8>,
) {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let key = plain_libs::utils::base64::base64_decode(&core.native_url_token().unwrap());
    (directory, core, state, key)
}

fn file_id(state: &ServerState, path: &str) -> String {
    plain_rs::features::files::resource_link::seal(
        &plain_rs::features::files::resource_link::Scope::Session(
            state.native_client_id().unwrap(),
        ),
        &plain_rs::features::sessions::token_key(&state.native_url_token().unwrap()).unwrap(),
        &plain_rs::features::files::resource_link::Resource::reference(path),
    )
    .unwrap()
}

fn temp_bin(state: &ServerState) -> (plain_libs::temporary::TempDir, String) {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let path = dir.path().join("movie.bin");
    std::fs::write(&path, b"0123456789").unwrap();
    (dir, file_id(state, &path.to_string_lossy()))
}

async fn call_fs(state: &ServerState, query: &str, headers: &[(&HeaderName, &str)]) -> Response {
    let mut builder = Request::get(format!("/fs?{query}"));
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let req = builder.body(Body::empty()).unwrap();
    super::get(plain_libs::axum::extract::State(state.clone()), req).await
}

async fn body_bytes(resp: Response) -> Vec<u8> {
    plain_libs::axum::body::to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .expect("body")
        .to_vec()
}

fn range_header(v: &str) -> (&'static HeaderName, &str) {
    (&header::RANGE, v)
}

#[tokio::test]
async fn fs_icons_return_original_bytes_for_all_preview_sizes() {
    let (_root, _core, state, _key) = test_state();
    let files = plain_libs::temporary::tempdir().unwrap();
    let original = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../assets/web/favicon.ico"
    ));
    for name in ["favicon.ico", "favicon.ICO"] {
        let path = files.path().join(name);
        std::fs::write(&path, original).unwrap();
        let id = file_id(&state, path.to_str().unwrap());
        for size in ["", "&size=SM", "&size=LG"] {
            for download in [false, true] {
                let query = format!(
                    "id={id}&action={}{}",
                    if download { "DOWNLOAD" } else { "PREVIEW" },
                    if download { "" } else { size }
                );
                let response = call_fs(&state, &query, &[]).await;
                assert_eq!(response.status(), StatusCode::OK, "{name}{size}");
                assert_eq!(response.headers()[header::CONTENT_TYPE], "image/x-icon");
                assert_eq!(
                    response.headers()[header::CONTENT_DISPOSITION]
                        .to_str()
                        .unwrap()
                        .starts_with("attachment;"),
                    download
                );
                assert_eq!(body_bytes(response).await, original);
                let response = call_fs(&state, &query, &[range_header("bytes=0-5")]).await;
                assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
                assert_eq!(response.headers()[header::CONTENT_TYPE], "image/x-icon");
                assert_eq!(body_bytes(response).await, &original[..6]);
            }
        }
    }
    assert_eq!(
        state
            .thumbnails
            .native_decodes
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
}

#[tokio::test]
async fn fs_animated_png_preserves_frames_for_both_extensions_and_preview_sizes() {
    use plain_libs::image::AnimationDecoder;
    let (_root, _core, state, _key) = test_state();
    let files = plain_libs::temporary::tempdir().unwrap();
    let original = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plain-rs/testdata/two-frame.apng"
    ));
    let frames = plain_libs::image::codecs::png::PngDecoder::new(std::io::Cursor::new(original))
        .unwrap()
        .apng()
        .unwrap()
        .into_frames()
        .collect_frames()
        .unwrap();
    assert_eq!(frames.len(), 2);
    assert_ne!(frames[0].buffer(), frames[1].buffer());
    for (name, mime) in [
        ("animation.png", "image/png"),
        ("animation.apng", "image/apng"),
    ] {
        let path = files.path().join(name);
        std::fs::write(&path, original).unwrap();
        let id = file_id(&state, path.to_str().unwrap());
        for options in ["PREVIEW", "PREVIEW&size=SM", "PREVIEW&size=LG", "DOWNLOAD"] {
            let response = call_fs(&state, &format!("id={id}&action={options}"), &[]).await;
            assert_eq!(response.status(), StatusCode::OK, "{name}{options}");
            assert_eq!(response.headers()[header::CONTENT_TYPE], mime);
            assert_eq!(body_bytes(response).await, original, "{name}{options}");
        }
    }
    assert_eq!(
        state
            .thumbnails
            .native_decodes
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
}

#[tokio::test]
async fn fs_bmp_previews_decode_and_downloads_keep_original_bytes() {
    let (_root, _core, state, _key) = test_state();
    let files = plain_libs::temporary::tempdir().unwrap();
    let original = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plain-rs/testdata/bmp/rgb24.bmp"
    ));
    let path = files.path().join("photo.bmp");
    std::fs::write(&path, original).unwrap();
    let id = file_id(&state, path.to_str().unwrap());
    let response = call_fs(&state, &format!("id={id}&action=PREVIEW"), &[]).await;
    assert_eq!(response.headers()[header::CONTENT_TYPE], "image/bmp");
    assert_eq!(body_bytes(response).await, original);
    for options in ["&size=SM", "&size=LG"] {
        let response = call_fs(&state, &format!("id={id}&action=PREVIEW{options}"), &[]).await;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = body_bytes(response).await;
        let decoded = plain_libs::image::load_from_memory(&bytes).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (3, 2));
        assert_eq!(
            call_fs(&state, &format!("id={id}&action=PREVIEW{options}"), &[])
                .await
                .status(),
            StatusCode::OK
        );
    }
    let response = call_fs(&state, &format!("id={id}&action=DOWNLOAD"), &[]).await;
    assert_eq!(response.headers()[header::CONTENT_TYPE], "image/bmp");
    assert_eq!(body_bytes(response).await, original);
    assert_eq!(
        state
            .thumbnails
            .native_decodes
            .load(std::sync::atomic::Ordering::Relaxed),
        2
    );
}

#[tokio::test]
async fn fs_bmp_alpha_survives_uncropped_previews_and_cached_requests() {
    let (_root, _core, state, _key) = test_state();
    let files = plain_libs::temporary::tempdir().unwrap();
    let original = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plain-rs/testdata/bmp/alpha32.bmp"
    ));
    let path = files.path().join("alpha.BMP");
    std::fs::write(&path, original).unwrap();
    let id = file_id(&state, path.to_str().unwrap());
    for size in ["SM", "LG"] {
        let query = format!("id={id}&action=PREVIEW&size={size}");
        let response = call_fs(&state, &query, &[]).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
        let etag = response.headers()[header::ETAG]
            .to_str()
            .unwrap()
            .to_owned();
        let bytes = body_bytes(response).await;
        let decoded = plain_libs::image::load_from_memory(&bytes)
            .unwrap()
            .into_rgba8();
        assert_eq!(decoded.dimensions(), (3, 1));
        assert_eq!(
            decoded.into_raw(),
            [255, 0, 0, 0, 0, 255, 0, 128, 0, 0, 255, 255]
        );
        let cached = call_fs(&state, &query, &[]).await;
        assert_eq!(body_bytes(cached).await, bytes);
        let conditional = call_fs(&state, &query, &[(&header::IF_NONE_MATCH, &etag)]).await;
        assert_eq!(conditional.status(), StatusCode::NOT_MODIFIED);
    }
    let original_response = call_fs(
        &state,
        &format!("id={id}&action=DOWNLOAD"),
        &[range_header("bytes=0-5")],
    )
    .await;
    assert_eq!(original_response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        original_response.headers()[header::CONTENT_TYPE],
        "image/bmp"
    );
    assert_eq!(body_bytes(original_response).await, &original[..6]);
    assert_eq!(
        state
            .thumbnails
            .native_decodes
            .load(std::sync::atomic::Ordering::Relaxed),
        2
    );
}

#[tokio::test]
async fn fs_range_serves_206_with_content_range() {
    let (_root, _core, state, _key) = test_state();
    let (_dir, id) = temp_bin(&state);
    let resp = call_fs(
        &state,
        &format!("id={id}&action=PREVIEW"),
        &[range_header("bytes=2-5")],
    )
    .await;
    assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        resp.headers().get(header::CONTENT_RANGE).unwrap(),
        "bytes 2-5/10"
    );
    assert_eq!(resp.headers().get(header::CONTENT_LENGTH).unwrap(), "4");
    assert_eq!(resp.headers().get(header::ACCEPT_RANGES).unwrap(), "bytes");
    assert_eq!(body_bytes(resp).await, b"2345");
}

#[tokio::test]
async fn fs_range_open_end_and_suffix() {
    let (_root, _core, state, _key) = test_state();
    let (_dir, id) = temp_bin(&state);
    for (spec, expected) in [("bytes=7-", b"789".as_ref()), ("bytes=-2", b"89".as_ref())] {
        let resp = call_fs(
            &state,
            &format!("id={id}&action=PREVIEW"),
            &[range_header(spec)],
        )
        .await;
        assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT, "{spec}");
        assert_eq!(body_bytes(resp).await, expected, "{spec}");
    }
}

#[tokio::test]
async fn fs_unsatisfiable_range_is_416() {
    let (_root, _core, state, _key) = test_state();
    let (_dir, id) = temp_bin(&state);
    for spec in ["bytes=100-200", "bytes=10-", "bytes=-0"] {
        let resp = call_fs(
            &state,
            &format!("id={id}&action=PREVIEW"),
            &[range_header(spec)],
        )
        .await;
        assert_eq!(resp.status(), StatusCode::RANGE_NOT_SATISFIABLE, "{spec}");
        assert_eq!(
            resp.headers().get(header::CONTENT_RANGE).unwrap(),
            "bytes */10",
            "{spec}"
        );
        assert!(body_bytes(resp).await.is_empty(), "{spec}");
    }
}

#[tokio::test]
async fn fs_malformed_or_absent_range_serves_200_full() {
    let (_root, _core, state, _key) = test_state();
    let (_dir, id) = temp_bin(&state);
    for spec in ["bytes=zzz", "items=0-9", "bytes=5-2"] {
        let resp = call_fs(
            &state,
            &format!("id={id}&action=PREVIEW"),
            &[range_header(spec)],
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK, "{spec}");
        assert_eq!(body_bytes(resp).await, b"0123456789", "{spec}");
    }
    let resp = call_fs(&state, &format!("id={id}&action=PREVIEW"), &[]).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers().get(header::ACCEPT_RANGES).unwrap(), "bytes");
    assert_eq!(body_bytes(resp).await, b"0123456789");
}

#[tokio::test]
async fn fs_recent_tracked_once_per_view_not_per_chunk() {
    let (_root, _core, state, _key) = test_state();
    let (_dir, id) = temp_bin(&state);

    call_fs(
        &state,
        &format!("id={id}&action=PREVIEW"),
        &[range_header("bytes=4-9")],
    )
    .await;
    assert!(
        state
            .file_platform
            .recent_files
            .list(10)
            .unwrap()
            .is_empty()
    );

    call_fs(
        &state,
        &format!("id={id}&action=PREVIEW"),
        &[range_header("bytes=0-9")],
    )
    .await;
    assert_eq!(state.file_platform.recent_files.list(10).unwrap().len(), 1);

    call_fs(&state, &format!("id={id}&action=PREVIEW"), &[]).await;
    assert_eq!(state.file_platform.recent_files.list(10).unwrap().len(), 1);
}

#[cfg(feature = "media")]
#[tokio::test]
async fn fs_probe_reports_codec_json() {
    let (_root, _core, state, _key) = test_state();
    let path = concat!(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../plain-rs"),
        "/testdata/video-h264-high.mp4"
    );
    let id = file_id(&state, path);
    let resp = call_fs(&state, &format!("id={id}&action=PROBE"), &[]).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/json"
    );
    let body = body_bytes(resp).await;
    assert_eq!(body, br#"{"codec":"avc1"}"#.as_ref());
}

#[tokio::test]
async fn fs_serves_fid_uris_from_app_file_store() {
    let (_root, _core, state, _key) = test_state();

    let imported = plain_rs::features::chat::attachments::app_file_store::import_bytes(
        &state.db,
        &state.directory,
        b"fid-payload-bytes",
        "text/plain",
    )
    .expect("import");
    assert!(imported.fid_suffix.ends_with(".txt"));

    let uri = format!("fid:{}", imported.fid_suffix);
    let id = file_id(&state, &uri);

    let resp = call_fs(&state, &format!("id={id}&action=PREVIEW"), &[]).await;
    assert_eq!(resp.status(), plain_libs::axum::http::StatusCode::OK);
    assert_eq!(body_bytes(resp).await, b"fid-payload-bytes");
}

#[tokio::test]
async fn fs_typed_attachment_and_file_references_keep_downloads_and_ranges() {
    let (_root, _core, state, _key) = test_state();
    let files = plain_libs::temporary::tempdir().unwrap();
    let path = files.path().join("preview.txt");
    std::fs::write(&path, b"fid-payload-bytes").unwrap();
    let imported = plain_rs::features::chat::attachments::app_file_store::import_bytes(
        &state.db,
        &state.directory,
        b"fid-payload-bytes",
        "text/plain",
    )
    .unwrap();
    for reference in [
        format!("fid:{}", imported.fid_suffix),
        path.to_string_lossy().into_owned(),
    ] {
        let id = file_id(&state, &reference);
        let response = call_fs(
            &state,
            &format!("id={id}&action=DOWNLOAD"),
            &[range_header("bytes=4-10")],
        )
        .await;
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert!(
            response.headers()[header::CONTENT_DISPOSITION]
                .to_str()
                .unwrap()
                .starts_with("attachment;")
        );
        assert_eq!(body_bytes(response).await, b"payload");
    }
}

#[cfg(feature = "media")]
#[tokio::test]
async fn fs_typed_image_reference_returns_decodable_thumbnail() {
    let (_root, _core, state, _key) = test_state();
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plain-rs/testdata/v-prog420.jpg"
    ));
    let imported = plain_rs::features::chat::attachments::app_file_store::import_bytes(
        &state.db,
        &state.directory,
        bytes,
        "image/jpeg",
    )
    .unwrap();
    let id = file_id(&state, &format!("fid:{}", imported.fid_suffix));
    let response = call_fs(&state, &format!("id={id}&action=PREVIEW&size=SM"), &[]).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_bytes(response).await;
    let thumbnail = plain_libs::image::load_from_memory(&body).unwrap();
    assert_eq!((thumbnail.width(), thumbnail.height()), (160, 120));
    let repeated = call_fs(&state, &format!("id={id}&action=PREVIEW&size=SM"), &[]).await;
    assert_eq!(repeated.status(), StatusCode::OK);
    assert_eq!(body, body_bytes(repeated).await);
}

#[tokio::test]
async fn fs_empty_png_has_no_thumbnail_and_remains_downloadable() {
    let (_root, core, state, _key) = test_state();
    let directory = plain_libs::temporary::tempdir().unwrap();
    let path = directory.path().join("empty.png");
    std::fs::write(&path, []).unwrap();
    let id = file_id(&state, path.to_str().unwrap());
    for size in ["SM", "LG"] {
        let response = call_fs(&state, &format!("id={id}&action=PREVIEW&size={size}"), &[]).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(body_bytes(response).await.is_empty());
    }
    let response = call_fs(&state, &format!("id={id}&action=DOWNLOAD"), &[]).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(body_bytes(response).await.is_empty());
    core.stop_core().await;
}

#[tokio::test]
async fn fs_small_images_target_short_edge_without_cropping() {
    let (_root, core, state, _key) = test_state();
    let files = plain_libs::temporary::tempdir().unwrap();
    for (source, small, large) in [
        ((400, 1200), (170, 512), (341, 1024)),
        ((1200, 400), (512, 170), (1024, 341)),
        ((200, 1600), (64, 512), (128, 1024)),
        ((100, 300), (100, 300), (100, 300)),
        ((400, 400), (256, 256), (400, 400)),
    ] {
        let path = files.path().join(format!("{}x{}.png", source.0, source.1));
        plain_libs::image::RgbImage::from_fn(source.0, source.1, |column, row| {
            plain_libs::image::Rgb([
                u8::from(column < source.0 / 8) * 255,
                u8::from(row < source.1 / 8) * 255,
                0,
            ])
        })
        .save(&path)
        .unwrap();
        let id = file_id(&state, path.to_str().unwrap());
        for (size, expected) in [("SM", small), ("LG", large), ("SM", small)] {
            let response = call_fs(&state, &format!("id={id}&action=PREVIEW&size={size}"), &[])
                .await;
            assert_eq!(response.status(), StatusCode::OK);
            let image = plain_libs::image::load_from_memory(&body_bytes(response).await)
                .unwrap()
                .into_rgb8();
            assert_eq!((image.width(), image.height()), expected, "{source:?} {size}");
            assert_eq!(image.get_pixel(0, 0).0, [255, 255, 0]);
            assert_eq!(
                image.get_pixel(image.width() - 1, image.height() - 1).0,
                [0, 0, 0]
            );
        }
    }
    core.stop_core().await;
}

#[tokio::test]
async fn fs_readable_images_share_authorization_for_paths_media_and_preview_sizes() {
    use plain_rs::features::files::resource_link::{self, Resource, ResourceType, Scope};
    let (_root, core, state, key) = test_state();
    let files = plain_libs::temporary::tempdir().unwrap();
    let library = files.path().join("Fixture.imovielibrary");
    std::fs::create_dir(&library).unwrap();
    let session = plain_rs::features::services::sessions::create(&state, "reader".into()).unwrap();
    let remote_key = plain_libs::utils::base64::base64_decode(&session.url_token);
    for extension in ["webp", "png", "svg"] {
        let path = library.join(format!("sample.{extension}"));
        if extension == "svg" {
            std::fs::write(&path, br##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="48"><rect width="64" height="48" fill="#123456"/></svg>"##).unwrap();
        } else {
            plain_libs::image::RgbImage::from_fn(64, 48, |x, y| {
                plain_libs::image::Rgb([x as u8, y as u8, 97])
            }).save(&path).unwrap();
        }
        let original = std::fs::read(&path).unwrap();
        let row = state.db.record_file_paths(
            &[path.to_string_lossy().into_owned()],
            Some(plain_rs::infrastructure::db::FileKind::Image),
        ).unwrap().remove(0);
        for resource in [Resource::reference(path.to_str().unwrap()), Resource { r#type: ResourceType::Media, id: row.id }] {
            for (scope, key) in [
                (Scope::Session(state.native_client_id().unwrap()), &key),
                (Scope::Session(session.client_id.clone()), &remote_key),
            ] {
                let id = resource_link::seal(&scope, key, &resource).unwrap();
                for size in ["", "&size=SM", "&size=LG"] {
                    let response = call_fs(&state, &format!("id={id}&action=PREVIEW{size}"), &[]).await;
                    assert_eq!(response.status(), StatusCode::OK, "{extension}{size} {resource:?} {scope:?}");
                    if extension == "svg" {
                        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/svg+xml");
                        assert!(response.headers()[header::CONTENT_SECURITY_POLICY].to_str().unwrap().contains("sandbox"));
                        assert_eq!(body_bytes(response).await, original);
                    } else {
                        let image = plain_libs::image::load_from_memory(&body_bytes(response).await).unwrap();
                        assert!(image.width() > 0 && image.height() > 0);
                    }
                }
                let response = call_fs(&state, &format!("id={id}&action=DOWNLOAD"), &[]).await;
                assert_eq!(response.status(), StatusCode::OK);
                assert_eq!(body_bytes(response).await, original);
            }
        }
    }
    core.stop_core().await;
}

#[cfg(unix)]
#[tokio::test]
async fn fs_cached_thumbnails_still_require_os_read_access() {
    use std::os::unix::fs::PermissionsExt;
    let (_root, _core, state, _key) = test_state();
    let files = plain_libs::temporary::tempdir().unwrap();
    let path = files.path().join("sample.png");
    plain_libs::image::RgbImage::from_fn(512, 384, |x, y| {
        plain_libs::image::Rgb([(x % 251) as u8, (y % 251) as u8, 97])
    }).save(&path).unwrap();
    let id = file_id(&state, path.to_str().unwrap());
    let query = format!("id={id}&action=PREVIEW&size=SM");
    assert_eq!(call_fs(&state, &query, &[]).await.status(), StatusCode::OK);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0)).unwrap();
    let response = call_fs(&state, &query, &[]).await;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(String::from_utf8_lossy(&body_bytes(response).await).contains("Permission denied"));
    assert_eq!(call_fs(&state, &query, &[]).await.status(), StatusCode::OK);
}

#[tokio::test]
async fn fs_private_application_paths_remain_forbidden_for_all_preview_sizes() {
    let (root, _core, state, _key) = test_state();
    let path = root.path().join("private.png");
    std::fs::write(&path, b"private fixture").unwrap();
    let id = file_id(&state, path.to_str().unwrap());
    for size in ["", "&size=SM", "&size=LG"] {
        assert_eq!(call_fs(&state, &format!("id={id}&action=PREVIEW{size}"), &[]).await.status(), StatusCode::FORBIDDEN);
    }
}

#[tokio::test]
async fn fs_missing_images_report_missing_regardless_of_preview_size() {
    let (_root, _core, state, _key) = test_state();
    let files = plain_libs::temporary::tempdir().unwrap();
    let path = files.path().join("Fixture.imovielibrary/missing.png");
    let id = file_id(&state, path.to_str().unwrap());
    for size in ["", "&size=SM", "&size=LG"] {
        assert_eq!(call_fs(&state, &format!("id={id}&action=PREVIEW{size}"), &[]).await.status(), StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn fs_rejects_missing_and_invalid_links_before_reading_files() {
    let (_root, _core, state, _key) = test_state();
    assert_eq!(
        call_fs(&state, "", &[]).await.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call_fs(&state, "id=invalid&action=PREVIEW", &[])
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn fs_download_ranges_preserve_original_bytes_and_reject_retired_slice_parameters() {
    let (_root, _core, state, _key) = test_state();
    let (_dir, id) = temp_bin(&state);
    let response = call_fs(
        &state,
        &format!("id={id}&action=DOWNLOAD"),
        &[range_header("bytes=8-13")],
    )
    .await;
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes 8-9/10");
    assert!(
        response.headers()[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .starts_with("attachment;")
    );
    assert_eq!(body_bytes(response).await, b"89");
    for extra in [
        "offset=8&length=6",
        "s=sm",
        "dl=1",
        "preview=markdown",
        "probe=1",
        "tr=1",
        "sid=share",
    ] {
        assert_eq!(
            call_fs(&state, &format!("id={id}&action=PREVIEW&{extra}"), &[])
                .await
                .status(),
            StatusCode::BAD_REQUEST
        );
    }
}

#[tokio::test]
async fn fs_share_links_use_the_share_key_and_become_unavailable_after_deletion() {
    let (_root, _core, state, _key) = test_state();
    let dir = plain_libs::temporary::tempdir().unwrap();
    let path = dir.path().join("shared.txt");
    std::fs::write(&path, b"shared-payload").unwrap();
    let shares =
        plain_rs::features::files::shares::Service::new(state.db.clone(), state.prefs.clone());
    let share = shares
        .create(plain_rs::features::files::shares::Input {
            name: "fixture".into(),
            password: String::new(),
            data: (vec![path.to_string_lossy().into_owned()])
                .into_iter()
                .map(|path: String| {
                    let path = std::path::PathBuf::from(path);
                    plain_rs::features::files::shares::SharePath {
                        virtual_path: format!("/{}", path.file_name().unwrap().to_str().unwrap()),
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
            id: "shared.txt".into(),
        },
    )
    .unwrap();
    let query = format!("id={id}&action=DOWNLOAD");
    let response = call_fs(&state, &query, &[range_header("bytes=0-5")]).await;
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert!(
        response.headers()[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .starts_with("attachment;")
    );
    assert_eq!(body_bytes(response).await, b"shared");
    assert_eq!(
        call_fs(&state, &format!("id={id}&action=PREVIEW"), &[])
            .await
            .status(),
        StatusCode::OK
    );
    state.db.share_delete(&share.id).unwrap();
    assert_eq!(
        call_fs(&state, &query, &[]).await.status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn fs_thumbnail_conditionals_and_downloads_keep_their_http_contract() {
    let (_root, _core, state, _key) = test_state();
    let images = plain_libs::temporary::tempdir().unwrap();
    let path = images.path().join("preview.png");
    plain_libs::image::RgbImage::new(64, 32)
        .save(&path)
        .unwrap();
    let original = std::fs::read(&path).unwrap();
    let id = file_id(&state, path.to_str().unwrap());
    let query = format!("id={id}&action=PREVIEW&size=SM");
    let response = call_fs(&state, &query, &[]).await;
    assert_eq!(response.status(), StatusCode::OK);
    let etag = response.headers()[header::ETAG]
        .to_str()
        .unwrap()
        .to_owned();
    let response = call_fs(&state, &query, &[(&header::IF_NONE_MATCH, &etag)]).await;
    assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(response.headers()[header::ETAG], etag);
    assert!(body_bytes(response).await.is_empty());
    let response = call_fs(&state, &format!("id={id}&action=DOWNLOAD"), &[]).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response.headers()[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .starts_with("attachment;")
    );
    assert_eq!(body_bytes(response).await, original);
}

#[tokio::test]
async fn session_urls_revoke_new_requests_without_canceling_started_transfers() {
    use plain_rs::features::files::resource_link::{self, Resource, Scope};
    let directory = plain_libs::temporary::tempdir().unwrap();
    let data = directory.path().join("data");
    std::fs::create_dir(&data).unwrap();
    let prefs = std::sync::Arc::new(
        plain_rs::features::prefs::Prefs::load(&data.join("prefs.json")).unwrap(),
    );
    let server = ContentServer::start(&data, &plain_libs::crypto::gen_token(), prefs).unwrap();
    let state = server.runtime_state();
    let mut first = plain_rs::features::services::sessions::create(&state, "first".into()).unwrap();
    state.db.session_delete(&first.client_id).unwrap();
    first.client_id = "706c6169-29b4-8d36-a1d3-e2ea4a2f0d38".into();
    state.db.session_save(&first).unwrap();
    let second = plain_rs::features::services::sessions::create(&state, "second".into()).unwrap();
    assert_ne!(first.url_token, second.url_token);
    assert_ne!(first.url_token, first.token);
    let bytes = vec![0x5a; 8 * 1024 * 1024];
    let file = directory.path().join("large.bin");
    std::fs::write(&file, &bytes).unwrap();
    let resource = Resource::reference(file.to_str().unwrap());
    let key = plain_libs::utils::base64::base64_decode(&first.url_token);
    let id =
        resource_link::seal(&Scope::Session(first.client_id.clone()), &key, &resource).unwrap();
    let native = resource_link::seal(
        &Scope::Session(state.native_client_id().unwrap()),
        &plain_libs::utils::base64::base64_decode(&server.native_url_token().unwrap()),
        &resource,
    )
    .unwrap();
    let url = format!("http://127.0.0.1:{}/fs?action=PREVIEW", server.port);
    let client = http_client::builder().build().unwrap();
    let response = client
        .get(&url)
        .query(&[("id", &id)])
        .header("Range", "bytes=1-3")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.bytes().await.unwrap().as_ref(), &bytes[1..4]);
    let (_, payload) = resource_link::parse(&id).unwrap();
    let forged = format!("c.{}.{payload}", second.client_id);
    assert_eq!(
        client
            .get(&url)
            .query(&[("id", forged)])
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let started = client.get(&url).query(&[("id", &id)]).send().await.unwrap();
    assert_eq!(started.status(), StatusCode::OK);
    plain_rs::features::services::sessions::delete(&state, &first.client_id).unwrap();
    assert_eq!(started.bytes().await.unwrap().as_ref(), bytes.as_slice());
    for range in [None, Some("bytes=4-7")] {
        let mut request = client.get(&url).query(&[("id", &id)]);
        if let Some(range) = range {
            request = request.header("Range", range);
        }
        assert_eq!(
            request.send().await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
    }
    let mut replacement =
        plain_rs::features::services::sessions::create(&state, "replacement".into()).unwrap();
    state.db.session_delete(&replacement.client_id).unwrap();
    replacement.client_id = first.client_id.clone();
    state.db.session_save(&replacement).unwrap();
    assert_eq!(
        client
            .get(&url)
            .query(&[("id", &id)])
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let fresh = resource_link::seal(
        &Scope::Session(replacement.client_id),
        &plain_libs::utils::base64::base64_decode(&replacement.url_token),
        &resource,
    )
    .unwrap();
    assert_eq!(
        client
            .get(&url)
            .query(&[("id", fresh)])
            .header("Range", "bytes=0-0")
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::PARTIAL_CONTENT
    );
    assert_eq!(
        client
            .get(&url)
            .query(&[("id", native)])
            .header("Range", "bytes=0-0")
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::PARTIAL_CONTENT
    );
    for internal in [data.join("prefs.json"), data.join("plain.db")] {
        let id = resource_link::seal(
            &Scope::Session(state.native_client_id().unwrap()),
            &plain_libs::utils::base64::base64_decode(&server.native_url_token().unwrap()),
            &Resource::reference(internal.to_str().unwrap()),
        )
        .unwrap();
        assert_eq!(
            client
                .get(&url)
                .query(&[("id", id)])
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    server.shutdown().await;
}

#[tokio::test]
async fn native_file_preview_uses_its_session_and_the_shared_preview() {
    use plain_rs::features::files::resource_link::{self, Resource, Scope};
    let (_root, core, state, _key) = test_state();
    let directory = plain_libs::temporary::tempdir().unwrap();
    let native =
        plain_rs::features::sessions::token_key(&core.runtime_state().native_url_token().unwrap())
            .unwrap();
    let scope = Scope::Session(state.native_client_id().unwrap());
    for (name, bytes, expected) in [
        ("fixture.txt", b"text preview\n".as_slice(), "text preview"),
        (
            "fixture.md",
            b"# Markdown preview\n".as_slice(),
            "# Markdown preview",
        ),
        (
            "fixture.docx",
            include_bytes!("../../../../plain-rs/testdata/preview.docx").as_slice(),
            "# Document preview",
        ),
    ] {
        let path = directory.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        let resource = Resource::reference(path.to_str().unwrap());
        let id = resource_link::seal(&scope, &native, &resource).unwrap();
        let preview = core.preview_native_file(id.clone()).await.unwrap();
        assert_eq!(preview.status, 200);
        assert!(preview.content.contains(expected));
        assert!(preview.headers["content-disposition"].contains(name));
        let response = call_fs(&state, &format!("id={id}&action=PREVIEW"), &[]).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            String::from_utf8(body_bytes(response).await)
                .unwrap()
                .contains(expected)
        );
        let wrong = resource_link::seal(&scope, &[9; 32], &resource).unwrap();
        assert_eq!(core.preview_native_file(wrong).await.unwrap().status, 403);
    }
}

#[tokio::test]
#[ignore]
async fn fs_thumbnail_latency_benchmark() {
    let (_root, core, state, _key) = test_state();
    let source = plain_libs::temporary::tempdir().unwrap();
    let (certificate_pem, certificate_key) =
        plain_libs::tls::generate_pem(&["localhost".into()]).unwrap();
    let pem = String::from_utf8(certificate_pem).unwrap();
    let (_, port) = core
        .start_public(0, 0, pem.clone().into_bytes(), certificate_key)
        .await
        .unwrap();
    let http = http_client::builder()
        .add_root_certificate(http_client::Certificate::from_pem(pem.as_bytes()).unwrap())
        .build()
        .unwrap();
    let origin = format!("https://localhost:{port}");
    http.get(format!("{origin}/health")).send().await.unwrap();
    let mut reports = Vec::new();
    for (label, width, height, extension) in [
        ("JPEG 1024x768", 1024, 768, "jpg"),
        ("JPEG 4000x3000", 4000, 3000, "jpg"),
        ("PNG 1600x1200", 1600, 1200, "png"),
    ] {
        let original = source.path().join(format!("original.{extension}"));
        plain_libs::image::RgbImage::from_fn(width, height, |x, y| {
            plain_libs::image::Rgb([(x % 200) as u8, (y % 200) as u8, 90])
        })
        .save(&original)
        .unwrap();
        let mut cold = Vec::new();
        let mut warm = Vec::new();
        for index in 0..20 {
            let path = source.path().join(format!("{width}-{index}.{extension}"));
            std::fs::copy(&original, &path).unwrap();
            let id = file_id(&state, path.to_str().unwrap());
            let started = std::time::Instant::now();
            let response = http
                .get(format!("{origin}/fs"))
                .query(&[("action", "PREVIEW")])
                .query(&[("id", id.as_str()), ("size", "SM")])
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let bytes = response.bytes().await.unwrap();
            cold.push(started.elapsed().as_secs_f64() * 1000.0);
            let image = plain_libs::image::load_from_memory(&bytes).unwrap();
            assert_eq!((image.width(), image.height()), (341, 256));
            let started = std::time::Instant::now();
            let response = http
                .get(format!("{origin}/fs"))
                .query(&[("action", "PREVIEW")])
                .query(&[("id", id.as_str()), ("size", "SM")])
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let cached = response.bytes().await.unwrap();
            warm.push(started.elapsed().as_secs_f64() * 1000.0);
            assert_eq!(bytes, cached);
        }
        cold.sort_by(f64::total_cmp);
        warm.sort_by(f64::total_cmp);
        reports.push(plain_libs::serde_json::json!({"input":label,"samples":20,"target":"256x192 aspect fit","coldMedianMs":cold[10],"coldP95Ms":cold[18],"coldMaxMs":cold[19],"warmMedianMs":warm[10],"warmP95Ms":warm[18],"warmMaxMs":warm[19]}));
    }
    println!(
        "{}",
        plain_libs::serde_json::json!({"transport":"trusted HTTPS HTTP/1.1 loopback, reused TLS connection","os":std::env::consts::OS,"architecture":std::env::consts::ARCH,"thumbnailCacheColdSourcePagesWarm":true,"reports":reports})
    );
    core.shutdown().await;
}

#[tokio::test]
async fn fs_300_cached_thumbnails_do_not_write_recent_files() {
    let (_root, core, state, _key) = test_state();
    let files = plain_libs::temporary::tempdir().unwrap();
    let path = files.path().join("thumbnail.jpg");
    std::fs::write(
        &path,
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/v-prog420.jpg"
        )),
    )
    .unwrap();
    let id = file_id(&state, path.to_str().unwrap());
    let query = format!("id={id}&action=PREVIEW&size=SM");
    let store = state.store.get().unwrap();
    let before = store.write_attempts_for_test();
    let response = call_fs(&state, &query, &[]).await;
    assert_eq!(response.status(), StatusCode::OK);
    let expected = body_bytes(response).await;
    let responses =
        futures_util::future::join_all((0..300).map(|_| call_fs(&state, &query, &[]))).await;
    for response in responses {
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(body_bytes(response).await, expected);
    }
    assert_eq!(
        store.write_attempts_for_test() - before,
        0,
        "thumbnail reads must not write user history"
    );
    assert!(
        state
            .file_platform
            .recent_files
            .list(10)
            .unwrap()
            .is_empty()
    );
    let view = call_fs(&state, &format!("id={id}&action=PREVIEW&size=LG"), &[]).await;
    assert_eq!(view.status(), StatusCode::OK);
    assert_eq!(store.write_attempts_for_test() - before, 0);
    let original = call_fs(&state, &format!("id={id}&action=PREVIEW"), &[]).await;
    assert_eq!(original.status(), StatusCode::OK);
    assert_eq!(store.write_attempts_for_test() - before, 0);
    assert_eq!(
        state.file_platform.recent_files.list(10).unwrap(),
        [path.to_str().unwrap()]
    );
    assert!(store.get("recent_files").unwrap().is_none());
    core.stop_core().await;
    assert_eq!(store.write_attempts_for_test() - before, 1);
    assert_eq!(
        store
            .get_json::<Vec<String>>("recent_files")
            .unwrap()
            .unwrap(),
        [path.to_str().unwrap()]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fs_serves_300_authenticated_media_hits_with_the_sqlite_connection_held() {
    use plain_rs::features::files::resource_link::{self, Resource, ResourceType, Scope};
    let (_root, core, state, _) = test_state();
    let files = plain_libs::temporary::tempdir().unwrap();
    let path = files.path().join("image.jpg");
    std::fs::write(
        &path,
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../plain-rs/testdata/v-prog420.jpg"
        )),
    )
    .unwrap();
    let row = state
        .db
        .record_file_paths(
            &[path.to_string_lossy().into_owned()],
            Some(plain_rs::infrastructure::db::FileKind::Image),
        )
        .unwrap()
        .remove(0);
    let client = plain_rs::features::services::sessions::create(&state, "reader".into()).unwrap();
    let id = resource_link::seal(
        &Scope::Session(client.client_id),
        &plain_libs::utils::base64::base64_decode(&client.url_token),
        &Resource {
            r#type: ResourceType::Media,
            id: row.id,
        },
    )
    .unwrap();
    let query = format!("id={id}&action=PREVIEW&size=SM");
    let first = call_fs(&state, &query, &[]).await;
    assert_eq!(first.status(), StatusCode::OK);
    let bytes = body_bytes(first).await;
    let db = state.db.clone();
    let (held, waiting) = tokio::sync::oneshot::channel();
    let (release, gate) = std::sync::mpsc::channel();
    let holder = std::thread::spawn(move || {
        db.with_conn(|_| {
            held.send(()).unwrap();
            gate.recv().unwrap();
        })
    });
    waiting.await.unwrap();
    let responses = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        futures_util::future::join_all((0..300).map(|_| call_fs(&state, &query, &[]))),
    )
    .await;
    let raw = format!("id={id}&action=PREVIEW");
    let originals = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        futures_util::future::join_all((0..51).map(|_| call_fs(&state, &raw, &[]))),
    )
    .await;
    release.send(()).unwrap();
    holder.join().unwrap();
    for response in originals.expect("view history waited for SQLite") {
        assert_eq!(response.status(), StatusCode::OK);
    }
    for response in responses.expect("media hits waited for SQLite") {
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(body_bytes(response).await, bytes);
    }
    core.stop_core().await;
}

#[tokio::test]
async fn fs_named_sizes_preserve_preview_quality_original_bytes_and_reject_invalid_values() {
    let (_root, core, state, _key) = test_state();
    let files = plain_libs::temporary::tempdir().unwrap();
    let path = files.path().join("sizes.jpg");
    plain_libs::image::RgbImage::from_fn(1600, 1200, |x, y| {
        plain_libs::image::Rgb([(x % 251) as u8, (y % 239) as u8, 90])
    })
    .save(&path)
    .unwrap();
    let id = file_id(&state, path.to_str().unwrap());
    for (size, dimensions) in [("SM", (341, 256)), ("LG", (1024, 768))] {
        let response = call_fs(&state, &format!("id={id}&action=PREVIEW&size={size}"), &[]).await;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = body_bytes(response).await;
        let decoded = plain_libs::image::load_from_memory(&bytes).unwrap();
        assert_eq!((decoded.width(), decoded.height()), dimensions);
    }
    let original = call_fs(&state, &format!("id={id}&action=PREVIEW"), &[]).await;
    assert_eq!(original.status(), StatusCode::OK);
    assert_eq!(body_bytes(original).await, std::fs::read(&path).unwrap());
    for size in ["", "md", "sm", "lg", "256", "sm%7Clg"] {
        let response = call_fs(&state, &format!("id={id}&action=PREVIEW&size={size}"), &[]).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{size}");
    }
    core.stop_core().await;
}

#[tokio::test]
async fn fs_png_previews_preserve_alpha_mime_and_conditional_cache_responses() {
    let (_root, core, state, _key) = test_state();
    let directory = plain_libs::temporary::tempdir().unwrap();
    let path = directory.path().join("transparent.png");
    plain_libs::image::RgbaImage::from_pixel(
        1200,
        900,
        plain_libs::image::Rgba([40, 90, 150, 128]),
    )
    .save(&path)
    .unwrap();
    let id = file_id(&state, path.to_str().unwrap());
    for (size, dimensions) in [("SM", (341, 256)), ("LG", (1024, 768))] {
        let query = format!("id={id}&action=PREVIEW&size={size}");
        let first = call_fs(&state, &query, &[]).await;
        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(first.headers()[header::CONTENT_TYPE], "image/png");
        let etag = first.headers()[header::ETAG].to_str().unwrap().to_owned();
        let bytes = body_bytes(first).await;
        let decoded = plain_libs::image::load_from_memory(&bytes)
            .unwrap()
            .into_rgba8();
        assert_eq!(decoded.dimensions(), dimensions);
        let pixel = decoded.get_pixel(dimensions.0 / 2, dimensions.1 / 2);
        assert_eq!(pixel[3], 128);
        for (actual, expected) in pixel.0[..3].iter().zip([40u8, 90, 150]) {
            assert!(actual.abs_diff(expected) <= 1);
        }
        let cached = call_fs(&state, &query, &[]).await;
        assert_eq!(body_bytes(cached).await, bytes);
        let conditional = call_fs(&state, &query, &[(&header::IF_NONE_MATCH, &etag)]).await;
        assert_eq!(conditional.status(), StatusCode::NOT_MODIFIED);
    }
    core.stop_core().await;
}

#[tokio::test]
async fn fs_tiff_avif_heic_and_heif_share_thumbnails_preserve_alpha_and_download_originals() {
    let (_root, core, state, _key) = test_state();
    let directory = plain_libs::temporary::tempdir().unwrap();
    for (extension, fixture, mime) in [
        ("tiff", "alpha.tiff", "image/tiff"),
        ("avif", "alpha.avif", "image/avif"),
        ("heic", "alpha.heic", "image/heic"),
        ("heif", "alpha.heic", "image/heic"),
    ] {
        let original = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../plain-rs/testdata/still-images")
                .join(fixture),
        )
        .unwrap();
        let path = directory.path().join(format!("alpha.{extension}"));
        std::fs::write(&path, &original).unwrap();
        let id = file_id(&state, path.to_str().unwrap());
        for size in ["SM", "LG"] {
            let query = format!("id={id}&action=PREVIEW&size={size}");
            let response = call_fs(&state, &query, &[]).await;
            assert_eq!(response.status(), StatusCode::OK, "{extension} {size}");
            assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
            let etag = response.headers()[header::ETAG]
                .to_str()
                .unwrap()
                .to_owned();
            let bytes = body_bytes(response).await;
            let pixels = plain_libs::image::load_from_memory(&bytes)
                .unwrap()
                .into_rgba8();
            assert_eq!(pixels.dimensions(), (64, 48), "{extension} {size}");
            assert!(pixels.pixels().any(|p| p[3] < 255), "{extension}");
            assert_eq!(body_bytes(call_fs(&state, &query, &[]).await).await, bytes);
            assert_eq!(
                call_fs(&state, &query, &[(&header::IF_NONE_MATCH, &etag)])
                    .await
                    .status(),
                StatusCode::NOT_MODIFIED
            );
        }
        let full = call_fs(&state, &format!("id={id}&action=PREVIEW"), &[]).await;
        assert_eq!(full.status(), StatusCode::OK, "{extension}");
        if extension == "avif" {
            assert_eq!(body_bytes(full).await, original);
        } else {
            assert_eq!(full.headers()[header::CONTENT_TYPE], "image/png");
            let pixels = plain_libs::image::load_from_memory(&body_bytes(full).await)
                .unwrap()
                .into_rgba8();
            assert_eq!(pixels.dimensions(), (64, 48));
            assert!(pixels.pixels().any(|p| p[3] < 255));
        }
        let download = call_fs(
            &state,
            &format!("id={id}&action=DOWNLOAD"),
            &[range_header("bytes=0-5")],
        )
        .await;
        assert_eq!(download.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(download.headers()[header::CONTENT_TYPE], mime);
        assert_eq!(body_bytes(download).await, &original[..6]);
    }
    assert_eq!(
        state
            .thumbnails
            .native_decodes
            .load(std::sync::atomic::Ordering::Relaxed),
        8
    );
    core.stop_core().await;
}

#[tokio::test]
async fn fs_small_browser_images_keep_dimensions_and_original_bytes_for_both_sizes() {
    let (_root, core, state, _key) = test_state();
    let directory = plain_libs::temporary::tempdir().unwrap();
    for (extension, mime) in [
        ("jpg", "image/jpeg"),
        ("png", "image/png"),
        ("webp", "image/webp"),
    ] {
        let path = directory.path().join(format!("small.{extension}"));
        plain_libs::image::RgbImage::from_fn(64, 48, |x, y| {
            plain_libs::image::Rgb([(x * 3) as u8, (y * 4) as u8, 90])
        })
        .save(&path)
        .unwrap();
        let original = std::fs::read(&path).unwrap();
        let id = file_id(&state, path.to_str().unwrap());
        for size in ["SM", "LG"] {
            let response =
                call_fs(&state, &format!("id={id}&action=PREVIEW&size={size}"), &[]).await;
            assert_eq!(response.status(), StatusCode::OK, "{extension} {size}");
            assert_eq!(response.headers()[header::CONTENT_TYPE], mime);
            let bytes = body_bytes(response).await;
            assert_eq!(bytes, original, "{extension} {size}");
            let pixels = plain_libs::image::load_from_memory(&bytes).unwrap();
            assert_eq!(
                (pixels.width(), pixels.height()),
                (64, 48),
                "{extension} {size}"
            );
        }
    }
    core.stop_core().await;
}

#[tokio::test]
async fn fs_large_still_formats_preserve_aspect_ratio_for_both_preview_sizes() {
    let (_root, core, state, _key) = test_state();
    let directory = plain_libs::temporary::tempdir().unwrap();
    for (extension, fixture) in [
        ("tiff", "grid.tiff"),
        ("avif", "grid.avif"),
        ("heic", "grid.heic"),
        ("heif", "grid.heic"),
    ] {
        let path = directory.path().join(format!("large.{extension}"));
        std::fs::copy(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../plain-rs/testdata/still-images")
                .join(fixture),
            &path,
        )
        .unwrap();
        let id = file_id(&state, path.to_str().unwrap());
        for (size, expected) in [("SM", (341, 256)), ("LG", (1024, 768))] {
            let response =
                call_fs(&state, &format!("id={id}&action=PREVIEW&size={size}"), &[]).await;
            assert_eq!(response.status(), StatusCode::OK, "{extension} {size}");
            let pixels = plain_libs::image::load_from_memory(&body_bytes(response).await).unwrap();
            assert_eq!(
                (pixels.width(), pixels.height()),
                expected,
                "{extension} {size}"
            );
        }
    }
    core.stop_core().await;
}

#[tokio::test]
async fn fs_document_preview_returns_markdown_and_preserves_original_downloads() {
    let (_root, _core, state, _key) = test_state();
    let files = plain_libs::temporary::tempdir().unwrap();
    let path = files.path().join("preview.docx");
    let original = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plain-rs/testdata/preview.docx"
    ));
    std::fs::write(&path, original).unwrap();
    let id = file_id(&state, path.to_str().unwrap());
    let response = call_fs(&state, &format!("id={id}&action=PREVIEW"), &[]).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/markdown; charset=utf-8"
    );
    let markdown = String::from_utf8(body_bytes(response).await).unwrap();
    assert!(markdown.contains("# Document preview"));
    assert!(markdown.contains("中文 & Markdown"));
    let response = call_fs(&state, &format!("id={id}&action=DOWNLOAD"), &[]).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_bytes(response).await, original);
    let private = state.directory.join("private.docx");
    std::fs::write(&private, original).unwrap();
    let id = file_id(&state, private.to_str().unwrap());
    let response = call_fs(&state, &format!("id={id}&action=PREVIEW"), &[]).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn fs_requires_one_uppercase_action_and_rejects_conflicting_parameters() {
    let (_root, _core, state, _key) = test_state();
    let (_files, id) = temp_bin(&state);
    for query in [
        format!("id={id}"),
        format!("id={id}&action=preview"),
        format!("id={id}&action=Preview"),
        format!("id={id}&action=download"),
        format!("id={id}&action=PREVIEW&size=sm"),
        format!("id={id}&action=unknown"),
        format!("id={id}&action=PREVIEW&action=DOWNLOAD"),
        format!("id={id}&action=DOWNLOAD&size=SM"),
        format!("id={id}&action=PROBE&size=LG"),
        format!("id={id}&action=PREVIEW&size=unknown"),
    ] {
        assert_eq!(
            call_fs(&state, &query, &[]).await.status(),
            StatusCode::BAD_REQUEST,
            "{query}"
        );
    }
}

#[tokio::test]
async fn fs_native_resource_ranges_use_the_provider_file_window() {
    let (_root, _core, state, _key) = test_state();
    let (generation, mut requests) = state.host.connect();
    let resource = "content://documents/preview";
    let id = file_id(&state, resource);
    let task = {
        let state = state.clone();
        tokio::spawn(async move {
            call_fs(
                &state,
                &format!("id={id}&action=DOWNLOAD"),
                &[range_header("bytes=2-5")],
            )
            .await
        })
    };
    let request = requests.recv().await.unwrap();
    assert_eq!(request["method"], "fileResourceStream");
    let stream = state
        .resources
        .attach_native(request["params"]["id"].as_str().unwrap(), generation)
        .unwrap();
    state
        .host
        .reply(
            generation,
            plain_libs::serde_json::json!({"id":request["id"],"result":null}),
        )
        .unwrap();
    let metadata: plain_libs::serde_json::Value =
        plain_libs::serde_json::from_str(&stream.next_request().await.unwrap().unwrap()).unwrap();
    assert_eq!(
        metadata,
        plain_libs::serde_json::json!({"path":resource,"file":true})
    );
    let mut file = plain_libs::temporary::tempfile().unwrap();
    std::io::Write::write_all(&mut file, b"PREFIX0123456789SUFFIX").unwrap();
    stream
        .file(
            plain_rs::infrastructure::host::resource_stream::NativeResourceFile::new(file, 6, 10)
                .unwrap(),
        )
        .await
        .unwrap();
    let response = task.await.unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes 2-5/10");
    assert_eq!(body_bytes(response).await, b"2345");
}

#[tokio::test]
async fn native_resources_use_memory_credentials_and_old_links_expire_after_restart() {
    let (root, core, state, _) = test_state();
    let files = plain_libs::temporary::tempdir().unwrap();
    let path = files.path().join("native.txt");
    std::fs::write(&path, b"native bytes").unwrap();
    let client_id = state.native_client_id().unwrap();
    assert!(state.db.session_list().unwrap().is_empty());
    assert_eq!(core.native_url_token().unwrap(), state.token.as_ref());
    let first = file_id(&state, path.to_str().unwrap());
    let response = call_fs(&state, &format!("id={first}&action=DOWNLOAD"), &[]).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_bytes(response).await, b"native bytes");
    let graphql = core.native_graphql().unwrap().execute("{ app { urlToken } sessions { clientId } }".into()).await;
    assert!(graphql.errors.is_empty(), "{:?}", graphql.errors);
    assert_eq!(graphql.data.into_json().unwrap()["sessions"], plain_libs::serde_json::json!([]));
    core.stop_core().await;
    assert!(core.native_url_token().is_err());
    drop(state);
    drop(core);
    let core = ContentServer::open(root.path()).unwrap();
    let state = core.runtime_state();
    assert_eq!(state.native_client_id().unwrap(), client_id);
    assert!(state.db.session_list().unwrap().is_empty());
    assert_eq!(call_fs(&state, &format!("id={first}&action=DOWNLOAD"), &[]).await.status(), StatusCode::FORBIDDEN);
    let current = file_id(&state, path.to_str().unwrap());
    let response = call_fs(&state, &format!("id={current}&action=DOWNLOAD"), &[]).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_bytes(response).await, b"native bytes");
    core.stop_core().await;
}
