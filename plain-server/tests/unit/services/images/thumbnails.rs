use crate::assembly::server::{ContentServer, ServerState};
use crate::services::images::thumbnails::Request;
use sha2::Digest;
use std::sync::{Arc, atomic::Ordering};

fn variant(base: &Request, center_crop: Option<bool>, if_none_match: Option<&str>) -> Request {
    Request {
        center_crop: center_crop.unwrap_or(base.center_crop),
        if_none_match: if_none_match.map(str::to_owned),
        ..Request {
            path: base.path.clone(),
            width: base.width,
            height: base.height,
            center_crop: base.center_crop,
            file_id: base.file_id.clone(),
            file_name: base.file_name.clone(),
            if_none_match: None,
        }
    }
}

fn request_for(path: &std::path::Path) -> Request {
    Request {
        path: path.to_str().unwrap().to_owned(),
        width: 80,
        height: 60,
        center_crop: true,
        file_id: String::new(),
        file_name: String::new(),
        if_none_match: None,
    }
}

async fn render(state: &ServerState, request: Request) -> (u16, axum::http::HeaderMap, Vec<u8>) {
    let response = crate::services::images::thumbnails::response(state, request).await;
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    (status, headers, bytes)
}

fn header<'a>(headers: &'a axum::http::HeaderMap, name: &str) -> String {
    headers.get(name).unwrap().to_str().unwrap().to_owned()
}

fn write_image(path: &std::path::Path, width: u32, height: u32) {
    image::RgbImage::from_fn(width, height, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
    })
    .save(path)
    .unwrap();
}

fn server_with_cache() -> (tempfile::TempDir, ContentServer, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Arc::new(plain_rs::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    prefs.set("client_id", "thumb-fixture").unwrap();
    let cache = dir.path().join("cache");
    let core = ContentServer::start(
        dir.path(),
        &plain_rs::utils::base64::base64_encode(&[4; 32]),
        prefs,
    )
    .unwrap();
    (dir, core, cache)
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn concurrent_requests_for_one_key_decode_once() {
    let dir = server_with_cache();
    let (_keep, core, _cache) = dir;
    let state = core.runtime_state();
    let source = state.directory.join("source.png");
    write_image(&source, 160, 80);

    let (_generation, mut host_requests) = state.host.connect();

    let mut calls = Vec::new();
    for _ in 0..8 {
        let state = state.clone();
        let source = source.clone();
        calls.push(tokio::spawn(async move {
            render(&state, request_for(&source)).await
        }));
    }
    let first = calls.remove(0).await.unwrap();
    for call in calls {
        assert_eq!(first, call.await.unwrap(), "every caller sees one render");
    }
    assert_eq!(
        state.thumbnails.native_decodes.load(Ordering::SeqCst),
        1,
        "concurrent requests for one key coalesce into a single decode;          first status {} body {:?}",
        first.0,
        String::from_utf8_lossy(&first.2)
    );
    assert_eq!(first.0, 200);
    assert_eq!(header(&first.1, "content-type"), "image/jpeg");
    assert!(host_requests.try_recv().is_err());
    assert_eq!(image::load_from_memory(&first.2).unwrap().width(), 80);
    assert_eq!(header(&first.1, "cache-control"), "private, max-age=86400");

    core.stop_core().await;
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn a_changed_source_and_a_different_shape_both_invalidate() {
    let (_dir, core, _cache) = server_with_cache();
    let state = core.runtime_state();
    let source = state.directory.join("source.png");
    write_image(&source, 160, 80);

    let base = request_for(&source);
    let first = render(&state, variant(&base, None, None)).await;
    assert_eq!(first.0, 200);

    let again = render(&state, variant(&base, None, None)).await;
    assert_eq!(again, first);
    assert_eq!(state.thumbnails.native_decodes.load(Ordering::SeqCst), 1);

    write_image(&source, 240, 100);
    let changed = render(&state, variant(&base, None, None)).await;
    assert_ne!(
        changed.2, first.2,
        "a changed source must not serve the old bytes"
    );

    let mut cropped = variant(&base, None, None);
    cropped.center_crop = false;
    let uncropped = render(&state, cropped).await;
    assert_ne!(
        uncropped.2, changed.2,
        "center-crop is part of the cache key"
    );

    core.stop_core().await;
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn a_matching_conditional_request_answers_not_modified() {
    let (_dir, core, _cache) = server_with_cache();
    let state = core.runtime_state();
    let source = state.directory.join("source.png");
    write_image(&source, 160, 80);

    let base = request_for(&source);
    let first = render(&state, variant(&base, None, None)).await;
    assert_eq!(first.0, 200);
    let etag = header(&first.1, "etag");
    assert_eq!(etag, format!("\"{}\"", hex(&first.2)));

    let mut conditional = variant(&base, None, None);
    conditional.if_none_match = Some(format!("\"old\", {etag}"));
    let not_modified = render(&state, conditional).await;
    assert_eq!(not_modified.0, 304);
    assert!(not_modified.2.is_empty(), "304 carries no body");

    let mut weak = variant(&base, None, None);
    weak.if_none_match = Some(format!("W/{etag}"));
    assert_eq!(render(&state, weak).await.0, 304);

    let mut other = variant(&base, None, None);
    other.center_crop = false;
    let other_etag = header(&render(&state, variant(&other, None, None)).await.1, "etag");
    let stale = variant(&other, None, Some(&etag));
    assert_eq!(render(&state, stale).await.0, 200);
    assert_ne!(other_etag, etag);
}

#[cfg(unix)]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn a_non_regular_source_is_refused_before_decoding() {
    let (_dir, core, _cache) = server_with_cache();
    let state = core.runtime_state();
    let (_generation, mut host_requests) = state.host.connect();
    let refused = render(&state, request_for(std::path::Path::new("/dev/null"))).await;
    assert_eq!(refused.0, 400);
    assert!(String::from_utf8_lossy(&refused.2).contains("regular thumbnail source required"));
    assert_eq!(state.thumbnails.native_decodes.load(Ordering::SeqCst), 0);
    assert!(host_requests.try_recv().is_err());
    core.stop_core().await;
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn a_source_that_vanishes_produces_not_found() {
    let (_dir, core, _cache) = server_with_cache();
    let state = core.runtime_state();
    let source = state.directory.join("missing.bin");
    let rendered = render(&state, request_for(&source)).await;
    assert_eq!(
        rendered.0, 404,
        "a missing thumbnail source preserves the resource not-found contract"
    );
    assert!(rendered.2.is_empty());
    core.stop_core().await;
}

fn hex(bytes: &[u8]) -> String {
    plain_rs::utils::hex::bytes_to_hex(&sha2::Sha256::digest(bytes))
}

#[tokio::test]
async fn native_thumbnail_output_is_one_shot_and_validates_before_consuming() {
    let thumbnails = super::Thumbnails::default();
    let (sender, mut receiver) = tokio::sync::oneshot::channel();
    thumbnails
        .outputs
        .lock()
        .unwrap()
        .insert("fixture".into(), sender);
    assert!(thumbnails.submit("fixture", Vec::new()).is_err());
    assert!(
        thumbnails
            .submit("fixture", vec![0; 32 * 1024 * 1024 + 1])
            .is_err()
    );
    thumbnails.submit("fixture", vec![0, 255, 1]).unwrap();
    assert_eq!(receiver.try_recv().unwrap(), vec![0, 255, 1]);
    assert!(thumbnails.submit("fixture", vec![1]).is_err());
    assert!(thumbnails.submit("expired", vec![1]).is_err());
}

#[test]
fn cancelled_thumbnail_output_discards_its_token() {
    let thumbnails = super::Thumbnails::default();
    let (sender, receiver) = tokio::sync::oneshot::channel();
    thumbnails
        .outputs
        .lock()
        .unwrap()
        .insert("cancelled".into(), sender);
    let output = super::Output {
        service: &thumbnails,
        token: "cancelled".into(),
    };
    drop(output);
    assert!(thumbnails.submit("cancelled", vec![1]).is_err());
    assert!(receiver.blocking_recv().is_err());
}

#[tokio::test]
async fn sdk_decode_validates_receipts_and_cleans_every_output_token() {
    for (receipt, output, expected) in [
        (
            serde_json::json!(true),
            Some(vec![0, 255, 1]),
            Ok(Some(vec![0, 255, 1])),
        ),
        (serde_json::json!(null), None, Ok(None)),
        (
            serde_json::json!(false),
            None,
            Err("invalid thumbnail receipt"),
        ),
        (
            serde_json::json!(true),
            None,
            Err("missing thumbnail output"),
        ),
    ] {
        let (_dir, core, _cache) = server_with_cache();
        let state = core.runtime_state();
        let (generation, mut requests) = state.host.connect();
        let owned = state.clone();
        let worker = tokio::spawn(async move {
            let request = requests.recv().await.unwrap();
            assert_eq!(request["method"], "thumbnailDecode");
            assert_eq!(request["params"]["mediaId"], "sdk-id");
            assert_eq!(request["params"]["width"], 80);
            let token = request["params"]["outputToken"].as_str().unwrap();
            if let Some(bytes) = output {
                owned.thumbnails.submit(token, bytes).unwrap();
            }
            owned
                .host
                .reply(
                    generation,
                    serde_json::json!({"id":request["id"],"result":receipt}),
                )
                .unwrap();
        });
        let mut request = request_for(std::path::Path::new("/image.jpg"));
        request.file_id = "sdk-id".into();
        let actual = super::decode_sdk(&state, &request, true).await;
        assert_eq!(
            actual
                .as_ref()
                .map(|value| value.clone())
                .map_err(String::as_str),
            expected
        );
        worker.await.unwrap();
        assert!(state.thumbnails.outputs.lock().unwrap().is_empty());
        core.stop_core().await;
    }
}

#[tokio::test]
async fn cancelling_a_pending_sdk_decode_removes_its_token() {
    let (_dir, core, _cache) = server_with_cache();
    let state = core.runtime_state();
    let (_generation, mut requests) = state.host.connect();
    let owned = state.clone();
    let call = tokio::spawn(async move {
        super::decode_sdk(
            &owned,
            &request_for(std::path::Path::new("/image.jpg")),
            false,
        )
        .await
    });
    let request = requests.recv().await.unwrap();
    let token = request["params"]["outputToken"].as_str().unwrap();
    call.abort();
    assert!(call.await.unwrap_err().is_cancelled());
    assert!(state.thumbnails.outputs.lock().unwrap().is_empty());
    assert!(state.thumbnails.submit(token, vec![1]).is_err());
    core.stop_core().await;
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::test]
async fn a_corrupt_native_image_returns_bad_request_with_a_json_decoder_error() {
    let (_directory, core, _cache) = server_with_cache();
    let state = core.runtime_state();
    let source = state.directory.join("corrupt.png");
    write_image(&source, 200, 100);
    let mut corrupt = std::fs::read(&source).unwrap();
    corrupt.truncate(33);
    std::fs::write(&source, &corrupt).unwrap();
    let (_generation, mut host_requests) = state.host.connect();
    let rendered = render(&state, request_for(&source)).await;
    assert_eq!(rendered.0, 400);
    assert_eq!(header(&rendered.1, "content-type"), "application/json");
    let error: serde_json::Value = serde_json::from_slice(&rendered.2).unwrap();
    let object = error.as_object().unwrap();
    assert_eq!(object.len(), 1);
    assert!(!object["error"].as_str().unwrap().is_empty());
    assert_eq!(state.thumbnails.native_decodes.load(Ordering::SeqCst), 1);
    assert!(host_requests.try_recv().is_err());
    assert_eq!(std::fs::read(&source).unwrap(), corrupt);
    core.stop_core().await;
}
