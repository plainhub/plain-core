use super::*;
use std::sync::Arc;

#[test]
fn cover_publication_is_content_addressed_and_repairs_incomplete_cache_files() {
    let directory = tempfile::tempdir().unwrap();
    let bytes = include_bytes!("../../../../../plain-rs/testdata/v-prog-gray.jpg");
    let first = publish(directory.path(), bytes).unwrap();
    assert_eq!(std::fs::read(&first).unwrap(), bytes);
    assert_eq!(publish(directory.path(), bytes).unwrap(), first);
    std::fs::write(&first, b"broken").unwrap();
    assert_eq!(publish(directory.path(), bytes).unwrap(), first);
    assert_eq!(std::fs::read(&first).unwrap(), bytes);
    assert_ne!(publish(directory.path(), b"other bytes").unwrap(), first);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
}

#[tokio::test]
async fn local_embedded_cover_uses_rust_without_host_metadata_and_has_a_nullable_absent_result() {
    let directory = tempfile::tempdir().unwrap();
    let host = Arc::new(plain_rs::host::bridge::Host::default());
    let (_generation, mut requests) = host.connect();
    let resources = plain_rs::host::resources::NativeResources::new(host);
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plain-rs/testdata/audio-cover.mp3"
    );
    let cover = cache(&resources, directory.path(), path)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        std::fs::read(cover).unwrap(),
        include_bytes!("../../../../../plain-rs/testdata/v-prog-gray.jpg")
    );
    let absent = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plain-rs/testdata/audio-untagged.mp3"
    );
    assert!(
        cache(&resources, directory.path(), absent)
            .await
            .unwrap()
            .is_none()
    );
    assert!(requests.try_recv().is_err());
}

#[tokio::test]
async fn asf_metadata_library_artwork_uses_the_shared_reader_without_platform_queries() {
    let directory = tempfile::tempdir().unwrap();
    let host = Arc::new(plain_rs::host::bridge::Host::default());
    let (_generation, mut requests) = host.connect();
    let resources = plain_rs::host::resources::NativeResources::new(host);
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plain-rs/testdata/audio-asf-library-cover.wma"
    );
    let cover = cache(&resources, directory.path(), path)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        std::fs::read(cover).unwrap(),
        include_bytes!("../../../../../plain-rs/testdata/v-prog-gray.jpg")
    );
    assert!(requests.try_recv().is_err());
}
