use super::*;

#[tokio::test]
async fn bounded_native_chunk_preserves_bytes_in_one_completed_chunk_file() {
    let (_root, server, uploads) = super::local_fixture::fixture();
    let directory = uploads.state.upload_chunks.directory();
    let bytes = (0..MAX_NATIVE_UPLOAD_CHUNK_BYTES)
        .map(|index| (index % 256) as u8)
        .collect::<Vec<_>>();
    uploads.write_chunk("fixture", 0, &bytes).await.unwrap();
    assert_eq!(
        std::fs::read(directory.join("fixture/chunk_0")).unwrap(),
        bytes
    );
    assert_eq!(
        std::fs::read_dir(directory.join("fixture"))
            .unwrap()
            .count(),
        1
    );
    server.stop_core().await;
}

#[tokio::test]
async fn empty_native_chunk_is_a_completed_zero_length_file() {
    let (_root, server, uploads) = super::local_fixture::fixture();
    let directory = uploads.state.upload_chunks.directory();
    uploads.write_chunk("fixture", 0, &[]).await.unwrap();
    assert_eq!(
        std::fs::metadata(directory.join("fixture/chunk_0"))
            .unwrap()
            .len(),
        0
    );
    server.stop_core().await;
}

#[tokio::test]
async fn oversized_native_chunk_is_rejected_before_creating_any_files() {
    let (_root, server, uploads) = super::local_fixture::fixture();
    let directory = uploads.state.upload_chunks.directory();
    assert!(
        uploads
            .write_chunk("fixture", 0, &vec![0; MAX_NATIVE_UPLOAD_CHUNK_BYTES + 1])
            .await
            .is_err()
    );
    assert!(!directory.join("fixture").exists());
    server.stop_core().await;
}
