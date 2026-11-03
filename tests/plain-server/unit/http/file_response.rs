use super::*;
use plain_rs::features::services::files::previews::Temporary;

#[tokio::test]
async fn thumbnail_body_shares_storage_and_releases_it_on_consumption_or_drop() {
    use std::sync::Arc;
    for consume in [true, false] {
        let storage: Arc<[u8]> = vec![0xff, 0xd8, 1, 2, 3].into();
        let pointer = storage.as_ptr();
        let owner = Arc::downgrade(&storage);
        let cached = axum::body::Bytes::from_owner(storage);
        let response = thumbnail(Ok(Some(cached.clone())), None);
        drop(cached);
        assert_eq!(owner.strong_count(), 1);
        if consume {
            let body = axum::body::to_bytes(response.into_body(), 1024)
                .await
                .unwrap();
            assert_eq!(body.as_ptr(), pointer);
            assert_eq!(body.as_ref(), &[0xff, 0xd8, 1, 2, 3]);
            drop(body);
        } else {
            drop(response);
        }
        assert!(owner.upgrade().is_none());
    }
}

#[test]
fn conditional_thumbnail_response_releases_its_shared_buffer() {
    use std::sync::Arc;
    let storage: Arc<[u8]> = vec![0xff, 0xd8, 1].into();
    let owner = Arc::downgrade(&storage);
    let response = thumbnail(Ok(Some(axum::body::Bytes::from_owner(storage))), Some("*"));
    assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
    assert!(owner.upgrade().is_none());
}

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

#[tokio::test]
async fn original_thumbnail_bytes_keep_their_mime_and_conditional_response() {
    for (bytes, mime) in [
        (b"\xff\xd8original".as_slice(), "image/jpeg"),
        (b"\x89PNGoriginal".as_slice(), "image/png"),
        (b"GIF89aoriginal".as_slice(), "image/gif"),
        (b"RIFF0000WEBPoriginal".as_slice(), "image/webp"),
    ] {
        let response = thumbnail(Ok(Some(axum::body::Bytes::from_static(bytes))), None);
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], mime);
        let etag = response.headers()[header::ETAG]
            .to_str()
            .unwrap()
            .to_owned();
        assert_eq!(
            &axum::body::to_bytes(response.into_body(), 1024)
                .await
                .unwrap()[..],
            bytes
        );
        let response = thumbnail(Ok(Some(axum::body::Bytes::from_static(bytes))), Some(&etag));
        assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
    }
}

#[tokio::test]
async fn mutable_file_responses_require_revalidation_and_preserve_full_and_range_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("preview.wav");
    for bytes in [b"broken-bytes", b"fixed--bytes"] {
        std::fs::write(&path, bytes).unwrap();
        for partial in [false, true] {
            let mut headers = HeaderMap::new();
            if partial {
                headers.insert(header::RANGE, "bytes=2-5".parse().unwrap());
            }
            let response = stream_file(&path, "audio/wav", &headers).await;
            assert_eq!(response.status(), if partial { StatusCode::PARTIAL_CONTENT } else { StatusCode::OK });
            assert_eq!(response.headers()[header::CACHE_CONTROL], "private, no-cache");
            let body = axum::body::to_bytes(response.into_body(), 1024).await.unwrap();
            assert_eq!(body.as_ref(), if partial { &bytes[2..6] } else { bytes });
        }
    }
}
