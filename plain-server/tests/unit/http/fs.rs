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
    plain_rs::files::resource_link::seal(
        &plain_rs::files::resource_link::Scope::Global,
        key,
        &plain_rs::files::resource_link::Resource::reference(path),
    )
    .unwrap()
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
async fn fs_typed_attachment_and_file_references_keep_downloads_and_ranges() {
    let (_root, _core, state, key) = test_state();
    let files = tempfile::tempdir().unwrap();
    let path = files.path().join("preview.txt");
    std::fs::write(&path, b"fid-payload-bytes").unwrap();
    let imported = plain_rs::chat::attachments::app_file_store::import_bytes(
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
        let id = file_id(&key, &reference);
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
                .starts_with("attachment;")
        );
        assert_eq!(body_bytes(response).await, b"payload");
    }
}

#[cfg(feature = "media")]
#[tokio::test]
async fn fs_typed_image_reference_returns_decodable_thumbnail() {
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
    let id = file_id(&key, &format!("fid:{}", imported.fid_suffix));
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
    let id = plain_rs::files::resource_link::seal(
        &plain_rs::files::resource_link::Scope::Share(share.id.clone()),
        &key,
        &plain_rs::files::resource_link::Resource {
            r#type: plain_rs::files::resource_link::ResourceType::SharedFile,
            id: "shared.txt".into(),
        },
    )
    .unwrap();
    let query = format!("id={id}&dl=1");
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
    let (_root, _core, state, key) = test_state();
    let images = tempfile::tempdir().unwrap();
    let path = images.path().join("preview.png");
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

#[tokio::test]
async fn session_urls_revoke_new_requests_without_cancelling_started_transfers() {
    use plain_rs::files::resource_link::{self, Resource, Scope};
    let directory = tempfile::tempdir().unwrap();
    let data = directory.path().join("data");
    std::fs::create_dir(&data).unwrap();
    let prefs =
        std::sync::Arc::new(plain_rs::prefs::Prefs::load(&data.join("prefs.json")).unwrap());
    let server = ContentServer::start(&data, &plain_rs::crypto::gen_token(), prefs).unwrap();
    let state = server.runtime_state();
    let mut first = plain_rs::services::sessions::create(&state, "first".into()).unwrap();
    state.db.session_delete(&first.client_id).unwrap();
    first.client_id = "客户.with.dots/?+#".into();
    state.db.session_save(&first).unwrap();
    let second = plain_rs::services::sessions::create(&state, "second".into()).unwrap();
    assert_ne!(first.url_token, second.url_token);
    assert_ne!(first.url_token, first.token);
    let bytes = vec![0x5a; 8 * 1024 * 1024];
    let file = directory.path().join("large.bin");
    std::fs::write(&file, &bytes).unwrap();
    let resource = Resource::reference(file.to_str().unwrap());
    let key = plain_rs::utils::base64::base64_decode(&first.url_token);
    let id =
        resource_link::seal(&Scope::Session(first.client_id.clone()), &key, &resource).unwrap();
    let global = resource_link::seal(
        &Scope::Global,
        &plain_rs::utils::base64::base64_decode(&server.native_url_token().unwrap()),
        &resource,
    )
    .unwrap();
    let url = format!("http://127.0.0.1:{}/fs", server.port);
    let client = reqwest::Client::new();
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
    plain_rs::services::sessions::delete(&state, &first.client_id).unwrap();
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
        plain_rs::services::sessions::create(&state, "replacement".into()).unwrap();
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
        &plain_rs::utils::base64::base64_decode(&replacement.url_token),
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
            .query(&[("id", global)])
            .header("Range", "bytes=0-0")
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::PARTIAL_CONTENT
    );
    for internal in [data.join("prefs.json"), data.join("plain.db")] {
        let id = resource_link::seal(
            &Scope::Global,
            &plain_rs::utils::base64::base64_decode(&server.native_url_token().unwrap()),
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
