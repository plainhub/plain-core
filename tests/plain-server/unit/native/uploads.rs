use super::*;

#[tokio::test]
async fn bounded_native_chunk_preserves_bytes_in_one_completed_chunk_file() {
    let directory = tempfile::tempdir().unwrap();
    let (stop, receiver) = watch::channel(false);
    let uploads = NativeUploads::new(
        Arc::new(UploadChunks::new(directory.path().to_owned())),
        receiver,
    );
    let bytes = (0..MAX_NATIVE_UPLOAD_CHUNK_BYTES)
        .map(|index| (index % 256) as u8)
        .collect::<Vec<_>>();
    uploads.write_chunk("fixture", 0, &bytes).await.unwrap();
    assert_eq!(
        std::fs::read(directory.path().join("fixture/chunk_0")).unwrap(),
        bytes
    );
    assert_eq!(
        std::fs::read_dir(directory.path().join("fixture"))
            .unwrap()
            .count(),
        1
    );
    drop(stop);
}

#[tokio::test]
async fn empty_native_chunk_is_a_completed_zero_length_file() {
    let directory = tempfile::tempdir().unwrap();
    let (_stop, receiver) = watch::channel(false);
    let uploads = NativeUploads::new(
        Arc::new(UploadChunks::new(directory.path().to_owned())),
        receiver,
    );
    uploads.write_chunk("fixture", 0, &[]).await.unwrap();
    assert_eq!(
        std::fs::metadata(directory.path().join("fixture/chunk_0"))
            .unwrap()
            .len(),
        0
    );
}

#[tokio::test]
async fn oversized_native_chunk_is_rejected_before_creating_any_files() {
    let directory = tempfile::tempdir().unwrap();
    let (_stop, receiver) = watch::channel(false);
    let uploads = NativeUploads::new(
        Arc::new(UploadChunks::new(directory.path().to_owned())),
        receiver,
    );
    assert!(
        uploads
            .write_chunk("fixture", 0, &vec![0; MAX_NATIVE_UPLOAD_CHUNK_BYTES + 1])
            .await
            .is_err()
    );
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}
