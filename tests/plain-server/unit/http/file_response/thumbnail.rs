use super::*;

#[tokio::test]
async fn thumbnail_body_shares_storage_and_releases_it_on_consumption_or_drop() {
    use std::sync::Arc;
    for consume in [true, false] {
        let storage: Arc<[u8]> = vec![0xff, 0xd8, 1, 2, 3].into();
        let pointer = storage.as_ptr();
        let owner = Arc::downgrade(&storage);
        let cached = plain_libs::axum::body::Bytes::from_owner(storage);
        let response = respond(Ok(Some(cached.clone())), None);
        drop(cached);
        assert_eq!(owner.strong_count(), 1);
        if consume {
            let body = plain_libs::axum::body::to_bytes(response.into_body(), 1024)
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
    let response = respond(
        Ok(Some(plain_libs::axum::body::Bytes::from_owner(storage))),
        Some("*"),
    );
    assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
    assert!(owner.upgrade().is_none());
}

#[tokio::test]
async fn original_thumbnail_bytes_keep_their_mime_and_conditional_response() {
    for (bytes, mime) in [
        (b"\xff\xd8original".as_slice(), "image/jpeg"),
        (b"\x89PNGoriginal".as_slice(), "image/png"),
        (b"GIF89aoriginal".as_slice(), "image/gif"),
        (b"RIFF0000WEBPoriginal".as_slice(), "image/webp"),
    ] {
        let response = respond(
            Ok(Some(plain_libs::axum::body::Bytes::from_static(bytes))),
            None,
        );
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], mime);
        let etag = response.headers()[header::ETAG]
            .to_str()
            .unwrap()
            .to_owned();
        assert_eq!(
            &plain_libs::axum::body::to_bytes(response.into_body(), 1024)
                .await
                .unwrap()[..],
            bytes
        );
        let response = respond(
            Ok(Some(plain_libs::axum::body::Bytes::from_static(bytes))),
            Some(&etag),
        );
        assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
    }
}
