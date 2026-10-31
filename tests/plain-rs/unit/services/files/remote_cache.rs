use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn destinations_preserve_safe_extensions_and_scope_stable_cache_keys_to_the_origin() {
    let root = Path::new("/cache");
    let first = destination(
        root,
        "https://example.invalid/fs?id=nonce-one",
        "image.jpg",
        Some("share/token/path/size"),
    )
    .unwrap();
    assert_eq!(
        first,
        destination(
            root,
            "https://example.invalid/fs?id=nonce-two",
            "image.jpg",
            Some("share/token/path/size")
        )
        .unwrap()
    );
    for (url, key) in [
        ("https://other.invalid/fs", "share/token/path/size"),
        ("https://example.invalid/fs", "share/other-token/path/size"),
        ("https://example.invalid/fs", "share/token/path/new-size"),
    ] {
        assert_ne!(
            first,
            destination(root, url, "image.jpg", Some(key)).unwrap()
        );
    }
    assert_eq!(first.extension().unwrap(), "jpg");
    let transient =
        destination(root, "https://example.invalid/a.mp4?token=opaque", "", None).unwrap();
    assert_eq!(transient.extension().unwrap(), "mp4");
    assert_ne!(
        transient,
        destination(root, "https://example.invalid/a.mp4?token=opaque", "", None).unwrap()
    );
    for name in [
        "../../escape",
        "bad.ext/escape",
        "name.ext?query",
        "name.ext;param",
    ] {
        let path = destination(root, "http://example.invalid/", name, Some("key")).unwrap();
        assert_eq!(path.parent(), Some(root));
        assert!(path.extension().is_none());
    }
    for url in [
        "file:///private/file",
        "content://provider/file",
        "ftp://example.invalid/file",
        "/relative",
    ] {
        assert!(destination(root, url, "file", None).is_err());
    }
}

#[tokio::test]
async fn concurrent_cached_downloads_fetch_once_and_publish_complete_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let cache = RemoteCache::new(directory.path().to_path_buf());
    let calls = AtomicUsize::new(0);
    let fetch = || async {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok((
            Some(6),
            futures_util::stream::iter(vec![
                Ok(bytes::Bytes::from_static(b"abc")),
                Ok(bytes::Bytes::from_static(b"def")),
            ]),
        ))
    };
    let (first, second) = tokio::join!(
        cache.download_with(
            directory.path(),
            "http://example.invalid/file?id=one",
            "image.png",
            Some("stable"),
            fetch
        ),
        cache.download_with(
            directory.path(),
            "http://example.invalid/file?id=two",
            "image.png",
            Some("stable"),
            fetch
        )
    );
    let first = first.unwrap();
    assert_eq!(first, second.unwrap());
    assert_eq!(tokio::fs::read(&first).await.unwrap(), b"abcdef");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    assert!(cache.client.get().is_none());
    assert_eq!(cache.permits.available_permits(), 4);
}

#[tokio::test]
async fn failed_and_truncated_downloads_publish_nothing_and_release_admission() {
    let directory = tempfile::tempdir().unwrap();
    let cache = RemoteCache::new(directory.path().to_path_buf());
    for (expected, chunks) in [
        (Some(6), vec![Ok(bytes::Bytes::from_static(b"short"))]),
        (Some(4), vec![Ok(bytes::Bytes::from_static(b"oversized"))]),
        (
            None,
            vec![
                Ok(bytes::Bytes::from_static(b"partial")),
                Err("network failed".into()),
            ],
        ),
    ] {
        assert!(
            cache
                .download_with(
                    directory.path(),
                    "http://example.invalid/file",
                    "photo.jpg",
                    Some("key"),
                    || async move { Ok((expected, futures_util::stream::iter(chunks))) }
                )
                .await
                .is_err()
        );
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
        assert_eq!(cache.permits.available_permits(), 4);
    }
}

#[tokio::test]
async fn cancellation_removes_partial_bytes_and_releases_the_cache_key() {
    let directory = tempfile::tempdir().unwrap();
    let cache = Arc::new(RemoteCache::new(directory.path().to_path_buf()));
    let (started, running) = tokio::sync::oneshot::channel();
    let path = directory.path().to_path_buf();
    let reader = cache.clone();
    let download = tokio::spawn(async move {
        reader
            .download_with(
                &path,
                "http://example.invalid/file",
                "photo.jpg",
                Some("key"),
                || async move {
                    let stream = futures_util::stream::once(async {
                        Ok(bytes::Bytes::from_static(b"partial"))
                    })
                    .chain(futures_util::stream::once(async move {
                        started.send(()).unwrap();
                        std::future::pending::<Result<bytes::Bytes, String>>().await
                    }));
                    Ok((None, stream))
                },
            )
            .await
    });
    running.await.unwrap();
    download.abort();
    assert!(download.await.unwrap_err().is_cancelled());
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    assert_eq!(cache.permits.available_permits(), 4);
    let result = cache
        .download_with(
            directory.path(),
            "http://example.invalid/file",
            "photo.jpg",
            Some("key"),
            || async {
                Ok((
                    Some(8),
                    futures_util::stream::iter(vec![Ok(bytes::Bytes::from_static(b"complete"))]),
                ))
            },
        )
        .await
        .unwrap();
    assert_eq!(tokio::fs::read(result).await.unwrap(), b"complete");
}

#[test]
fn cache_location_uses_the_supplied_cache_root_without_a_host() {
    let directory = tempfile::tempdir().unwrap();
    let cache = RemoteCache::new(directory.path().to_path_buf());
    assert_eq!(cache.directory(), directory.path().join("remote_files"));
}
