use crate::server::ContentServer;
use crate::server::ServerState;
use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderName, StatusCode, header};
use axum::response::Response;
use plain_rs::recent_files as recent;

fn test_state() -> (tempfile::TempDir, ContentServer, ServerState, Vec<u8>) {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let key = plain_rs::utils::base64::base64_decode(&core.native_url_token().unwrap());
    (directory, core, state, key)
}

fn file_id(key: &[u8], path: &str) -> String {
    let blob = plain_rs::crypto::xchacha_encrypt_raw(key, path.as_bytes()).expect("encrypt");
    plain_rs::utils::base64::base64_encode_url_safe(&blob)
}

fn temp_bin(key: &[u8]) -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("movie.bin");
    std::fs::write(&path, b"0123456789").unwrap();
    (dir, file_id(key, &path.to_string_lossy()))
}

async fn call_fs(state: &ServerState, query: &str, headers: &[(&HeaderName, &str)]) -> Response {
    let mut builder = Request::get(format!("/fs?{query}"));
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let req = builder.body(Body::empty()).unwrap();
    super::get(axum::extract::State(state.clone()), req).await
}

async fn body_bytes(resp: Response) -> Vec<u8> {
    axum::body::to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .expect("body")
        .to_vec()
}

fn range_header(v: &str) -> (&'static HeaderName, &str) {
    (&header::RANGE, v)
}

#[tokio::test]
async fn fs_range_serves_206_with_content_range() {
    let (_root, _core, state, key) = test_state();
    let (_dir, id) = temp_bin(&key);
    let resp = call_fs(&state, &format!("id={id}"), &[range_header("bytes=2-5")]).await;
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
    let (_root, _core, state, key) = test_state();
    let (_dir, id) = temp_bin(&key);
    for (spec, expected) in [("bytes=7-", b"789".as_ref()), ("bytes=-2", b"89".as_ref())] {
        let resp = call_fs(&state, &format!("id={id}"), &[range_header(spec)]).await;
        assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT, "{spec}");
        assert_eq!(body_bytes(resp).await, expected, "{spec}");
    }
}

#[tokio::test]
async fn fs_unsatisfiable_range_is_416() {
    let (_root, _core, state, key) = test_state();
    let (_dir, id) = temp_bin(&key);
    for spec in ["bytes=100-200", "bytes=10-", "bytes=-0"] {
        let resp = call_fs(&state, &format!("id={id}"), &[range_header(spec)]).await;
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
    let (_root, _core, state, key) = test_state();
    let (_dir, id) = temp_bin(&key);
    for spec in ["bytes=zzz", "items=0-9", "bytes=5-2"] {
        let resp = call_fs(&state, &format!("id={id}"), &[range_header(spec)]).await;
        assert_eq!(resp.status(), StatusCode::OK, "{spec}");
        assert_eq!(body_bytes(resp).await, b"0123456789", "{spec}");
    }
    let resp = call_fs(&state, &format!("id={id}"), &[]).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers().get(header::ACCEPT_RANGES).unwrap(), "bytes");
    assert_eq!(body_bytes(resp).await, b"0123456789");
}

#[tokio::test]
async fn fs_recent_tracked_once_per_view_not_per_chunk() {
    let (_root, _core, state, key) = test_state();
    let (_dir, id) = temp_bin(&key);

    call_fs(&state, &format!("id={id}"), &[range_header("bytes=4-9")]).await;
    assert!(
        recent::list(state.store.get().unwrap().as_ref(), 10)
            .unwrap()
            .is_empty()
    );

    call_fs(&state, &format!("id={id}"), &[range_header("bytes=0-9")]).await;
    assert_eq!(
        recent::list(state.store.get().unwrap().as_ref(), 10)
            .unwrap()
            .len(),
        1
    );

    call_fs(&state, &format!("id={id}"), &[]).await;
    assert_eq!(
        recent::list(state.store.get().unwrap().as_ref(), 10)
            .unwrap()
            .len(),
        1
    );
}

#[cfg(feature = "media")]
#[tokio::test]
async fn fs_probe_reports_codec_json() {
    let (_root, _core, state, key) = test_state();
    let path = concat!(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../plain-rs"),
        "/testdata/video-h264-high.mp4"
    );
    let id = file_id(&key, path);
    let resp = call_fs(&state, &format!("id={id}&probe=1"), &[]).await;
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
    let (_root, _core, state, key) = test_state();

    let imported = plain_rs::chat::attachments::app_file_store::import_bytes(
        &state.db,
        &state.directory,
        b"fid-payload-bytes",
        "text/plain",
    )
    .expect("import");
    assert!(imported.fid_suffix.ends_with(".txt"));

    let uri = format!("fid:{}", imported.fid_suffix);
    let id = file_id(&key, &uri);

    let resp = call_fs(&state, &format!("id={id}"), &[]).await;
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    assert_eq!(body_bytes(resp).await, b"fid-payload-bytes");
}

#[tokio::test]
async fn fs_serves_json_wrapped_references_with_names_and_ranges() {
    let (_root, _core, state, key) = test_state();
    let imported = plain_rs::chat::attachments::app_file_store::import_bytes(
        &state.db,
        &state.directory,
        b"fid-payload-bytes",
        "text/plain",
    )
    .unwrap();
    let uri = format!("fid:{}", imported.fid_suffix);
    for path in [&uri, "relative.txt"] {
        if path == "relative.txt" {
            std::fs::write(state.directory.join(path), b"fid-payload-bytes").unwrap();
        }
        let envelope = serde_json::json!({"path":path,"name":"preview.txt"}).to_string();
        let id = file_id(&key, &envelope);
        let response = call_fs(
            &state,
            &format!("id={id}&dl=1"),
            &[range_header("bytes=4-10")],
        )
        .await;
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        assert!(
            response.headers()[header::CONTENT_DISPOSITION]
                .to_str()
                .unwrap()
                .contains("preview.txt")
        );
        assert_eq!(body_bytes(response).await, b"payload");
    }
}

#[cfg(feature = "media")]
#[tokio::test]
async fn fs_json_wrapped_image_reference_returns_decodable_thumbnail() {
    let (_root, _core, state, key) = test_state();
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plain-rs/testdata/v-prog420.jpg"
    ));
    let imported = plain_rs::chat::attachments::app_file_store::import_bytes(
        &state.db,
        &state.directory,
        bytes,
        "image/jpeg",
    )
    .unwrap();
    let envelope =
        serde_json::json!({"path":format!("fid:{}", imported.fid_suffix),"name":"preview.jpg"})
            .to_string();
    let id = file_id(&key, &envelope);
    let response = call_fs(&state, &format!("id={id}&w=32&h=32"), &[]).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_bytes(response).await;
    let thumbnail = image::load_from_memory(&body).unwrap();
    assert!(thumbnail.width() > 0 && thumbnail.width() <= 32);
    assert!(thumbnail.height() > 0 && thumbnail.height() <= 32);
}

#[tokio::test]
async fn fs_rejects_missing_and_invalid_links_before_reading_files() {
    let (_root, _core, state, _key) = test_state();
    assert_eq!(
        call_fs(&state, "", &[]).await.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        call_fs(&state, "id=invalid", &[]).await.status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn fs_slice_queries_preserve_bounds_and_take_precedence_over_download_and_range() {
    let (_root, _core, state, key) = test_state();
    let (_dir, id) = temp_bin(&key);
    let response = call_fs(
        &state,
        &format!("id={id}&offset=8&length=6&dl=1"),
        &[range_header("bytes=0-1")],
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .headers()
            .get(header::CONTENT_DISPOSITION)
            .is_none()
    );
    assert_eq!(body_bytes(response).await, b"89");
    for (extra, expected) in [
        ("offset=-1&length=2", StatusCode::NOT_FOUND),
        ("offset=0&length=262145", StatusCode::BAD_REQUEST),
    ] {
        assert_eq!(
            call_fs(&state, &format!("id={id}&{extra}"), &[])
                .await
                .status(),
            expected
        );
    }
}

#[tokio::test]
async fn fs_share_links_use_the_share_key_and_become_unavailable_after_deletion() {
    let (_root, _core, state, _key) = test_state();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("shared.txt");
    std::fs::write(&path, b"shared-payload").unwrap();
    let shares = plain_rs::files::shares::Service::new(state.db.clone(), state.prefs.clone());
    let share = shares
        .create(
            "fixture".into(),
            vec![path.to_string_lossy().into_owned()],
            true,
            None,
        )
        .unwrap();
    let key = plain_rs::utils::base64::base64_decode(&share.url_token);
    let params = serde_json::json!({"sharedId":share.id,"virtualPath":"shared.txt"}).to_string();
    let id = file_id(&key, &params);
    let query = format!("id={id}&sid={}&dl=1", share.id);
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
        call_fs(&state, &format!("id={id}"), &[]).await.status(),
        StatusCode::FORBIDDEN
    );
    state.db.share_delete(&share.id).unwrap();
    assert_eq!(
        call_fs(&state, &query, &[]).await.status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn fs_thumbnail_conditionals_and_downloads_keep_their_http_contract() {
    let (_root, _core, state, key) = test_state();
    let path = state.directory.join("preview.png");
    image::RgbImage::new(64, 32).save(&path).unwrap();
    let original = std::fs::read(&path).unwrap();
    let id = file_id(&key, path.to_str().unwrap());
    let query = format!("id={id}&w=16&h=16");
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
    let response = call_fs(&state, &format!("{query}&dl=1"), &[]).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response.headers()[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .starts_with("attachment;")
    );
    assert_eq!(body_bytes(response).await, original);
}
