use super::*;

#[tokio::test]
async fn slices_preserve_short_reads_and_the_existing_limits() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.bin");
    std::fs::write(&path, b"0123456789").unwrap();
    for (offset, length, expected) in [(2, 4, b"2345".as_slice()), (8, 6, b"89"), (20, 2, b"")] {
        let Content::Bytes(bytes) = read_slice(path.to_str().unwrap(), offset, length)
            .await
            .unwrap()
        else {
            panic!("expected file bytes");
        };
        assert_eq!(bytes, expected);
    }
    assert!(matches!(
        read_slice(path.to_str().unwrap(), -1, 2).await.unwrap(),
        Content::NotFound
    ));
    assert!(matches!(
        read_slice(path.to_str().unwrap(), 0, 256 * 1024 + 1)
            .await
            .unwrap(),
        Content::SliceTooLarge
    ));
    assert!(matches!(
        read_slice(dir.path().join("missing").to_str().unwrap(), 0, 2)
            .await
            .unwrap(),
        Content::NotFound
    ));
}

#[tokio::test]
async fn photo_resource_uris_use_the_sdk_resource_stream_instead_of_filesystem_reads() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let path = "ph://asset/L0/001";
    links::authorize_path(&state, path).unwrap();
    let result = read(
        &state,
        links::File {
            path: path.into(),
            media_id: "photos:asset/L0/001".into(),
            name: "photo.png".into(),
        },
        Options {
            access_order: 0,
            download: true,
            offset: Some(0),
            length: Some(32),
            size: None,
            #[cfg(feature = "runtime-media")]
            pdf: false,
            probe: false,
            transcode: false,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            starts_view: false,
        },
    )
    .await
    .unwrap();
    assert!(
        matches!(result, Content::Resource { path: resource, download: true, name: Some(name) } if resource == path && name == "photo.png")
    );
    core.stop_core().await;
}
