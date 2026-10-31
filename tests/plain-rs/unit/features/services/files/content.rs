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
