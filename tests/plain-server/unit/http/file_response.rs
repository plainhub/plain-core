use super::*;
use plain_rs::services::files::previews::Temporary;

#[tokio::test]
async fn temporary_output_survives_until_the_response_body_is_consumed_or_cancelled() {
    let dir = tempfile::tempdir().unwrap();
    for consume in [true, false] {
        let path = dir.path().join("preview.png");
        std::fs::write(&path, b"preview-output").unwrap();
        let response =
            stream_temporary(Temporary(path.clone()), "image/png", &HeaderMap::new()).await;
        assert!(path.exists());
        if consume {
            let bytes = axum::body::to_bytes(response.into_body(), 1024)
                .await
                .unwrap();
            assert_eq!(&bytes[..], b"preview-output");
        } else {
            drop(response);
        }
        assert!(!path.exists(), "the response owns temporary-file cleanup");
    }
}

#[tokio::test]
async fn temporary_range_responses_keep_range_headers_and_remove_the_output() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("preview.png");
    std::fs::write(&path, b"0123456789").unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(header::RANGE, "bytes=2-5".parse().unwrap());
    let response = stream_temporary(Temporary(path.clone()), "image/png", &headers).await;
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes 2-5/10");
    let bytes = axum::body::to_bytes(response.into_body(), 1024)
        .await
        .unwrap();
    assert_eq!(&bytes[..], b"2345");
    assert!(!path.exists());
}
