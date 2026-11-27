use super::*;

#[tokio::test]
async fn temporary_output_survives_until_the_response_body_is_consumed_or_canceled() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    for consume in [true, false] {
        let path = dir.path().join("preview.png");
        std::fs::write(&path, b"preview-output").unwrap();
        let response = Source::Temporary(Temporary(path.clone()))
            .respond("image/png", &HeaderMap::new())
            .await;
        assert!(path.exists());
        if consume {
            let bytes = plain_libs::axum::body::to_bytes(response.into_body(), 1024)
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
    let dir = plain_libs::temporary::tempdir().unwrap();
    let path = dir.path().join("preview.png");
    std::fs::write(&path, b"0123456789").unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(header::RANGE, "bytes=2-5".parse().unwrap());
    let response = Source::Temporary(Temporary(path.clone()))
        .respond("image/png", &headers)
        .await;
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes 2-5/10");
    let bytes = plain_libs::axum::body::to_bytes(response.into_body(), 1024)
        .await
        .unwrap();
    assert_eq!(&bytes[..], b"2345");
    assert!(!path.exists());
}

#[tokio::test]
async fn mutable_file_responses_require_revalidation_and_preserve_full_and_range_bytes() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let path = dir.path().join("preview.wav");
    for bytes in [b"broken-bytes", b"fixed--bytes"] {
        std::fs::write(&path, bytes).unwrap();
        for partial in [false, true] {
            let mut headers = HeaderMap::new();
            if partial {
                headers.insert(header::RANGE, "bytes=2-5".parse().unwrap());
            }
            let response = Source::Path(path.clone())
                .respond("audio/wav", &headers)
                .await;
            assert_eq!(
                response.status(),
                if partial {
                    StatusCode::PARTIAL_CONTENT
                } else {
                    StatusCode::OK
                }
            );
            assert_eq!(
                response.headers()[header::CACHE_CONTROL],
                "private, no-cache"
            );
            let body = plain_libs::axum::body::to_bytes(response.into_body(), 1024)
                .await
                .unwrap();
            assert_eq!(body.as_ref(), if partial { &bytes[2..6] } else { bytes });
        }
    }
}

#[tokio::test]
async fn open_file_window_streams_multiple_frames_and_detects_premature_eof() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let path = dir.path().join("resource");
    let original: Vec<_> = (0..190_000).map(|n| (n % 251) as u8).collect();
    std::fs::write(&path, &original).unwrap();
    let file = std::fs::File::open(&path).unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(header::RANGE, "bytes=4-135999".parse().unwrap());
    let response = Source::Native(NativeResourceFile::new(file, 7, 180000).unwrap())
        .respond("video/mp4", &headers)
        .await;
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    let bytes = plain_libs::axum::body::to_bytes(response.into_body(), original.len())
        .await
        .unwrap();
    assert_eq!(&bytes[..], &original[11..136007]);

    let response = Source::Path(path.clone())
        .respond("video/mp4", &HeaderMap::new())
        .await;
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap()
        .set_len(70_000)
        .unwrap();
    assert!(
        plain_libs::axum::body::to_bytes(response.into_body(), original.len())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn range_body_transfers_the_read_buffer_directly_and_stops_reading_when_dropped() {
    use futures_util::StreamExt;
    use std::{
        io,
        sync::{Arc, Mutex},
    };

    struct ObservedFile {
        file: File,
        reads: Arc<Mutex<Vec<usize>>>,
    }
    impl Read for ObservedFile {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            self.reads.lock().unwrap().push(buffer.as_ptr() as usize);
            self.file.read(buffer)
        }
    }
    impl Seek for ObservedFile {
        fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
            self.file.seek(position)
        }
    }

    let dir = plain_libs::temporary::tempdir().unwrap();
    let path = dir.path().join("video");
    std::fs::write(&path, vec![7; 3 * 65536]).unwrap();
    let reads = Arc::new(Mutex::new(Vec::new()));
    let file = ObservedFile {
        file: File::open(path).unwrap(),
        reads: Arc::clone(&reads),
    };
    let response = range_response(
        file,
        0,
        3 * 65536,
        None,
        "video/mp4",
        Some(HeaderValue::from_static("bytes=64-")),
    );
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert!(reads.lock().unwrap().is_empty());
    let mut stream = response.into_body().into_data_stream();
    let first = stream.next().await.unwrap().unwrap();
    assert_eq!(first.len(), 65536);
    assert_eq!(reads.lock().unwrap().as_slice(), &[first.as_ptr() as usize]);
    drop(stream);
    assert_eq!(reads.lock().unwrap().len(), 1);
}
