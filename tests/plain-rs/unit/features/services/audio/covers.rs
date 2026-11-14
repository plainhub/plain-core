use super::*;
use std::sync::Arc;

#[tokio::test]
async fn local_embedded_cover_uses_rust_without_host_metadata_and_has_a_nullable_absent_result() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let host = Arc::new(crate::infrastructure::host::bridge::Host::default());
    let (_generation, mut requests) = host.connect();
    let resources = crate::infrastructure::host::resources::NativeResources::new(host);
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plain-rs/testdata/audio-cover.mp3"
    );
    let cover = read(&resources, directory.path(), path)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        cover,
        include_bytes!("../../../../../../plain-rs/testdata/v-prog-gray.jpg")
    );
    let absent = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plain-rs/testdata/audio-untagged.mp3"
    );
    assert!(
        read(&resources, directory.path(), absent)
            .await
            .unwrap()
            .is_none()
    );
    assert!(requests.try_recv().is_err());
}

#[tokio::test]
async fn asf_metadata_library_artwork_uses_the_shared_reader_without_platform_queries() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let host = Arc::new(crate::infrastructure::host::bridge::Host::default());
    let (_generation, mut requests) = host.connect();
    let resources = crate::infrastructure::host::resources::NativeResources::new(host);
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plain-rs/testdata/audio-asf-library-cover.wma"
    );
    let cover = read(&resources, directory.path(), path)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        cover,
        include_bytes!("../../../../../../plain-rs/testdata/v-prog-gray.jpg")
    );
    assert!(requests.try_recv().is_err());
}
