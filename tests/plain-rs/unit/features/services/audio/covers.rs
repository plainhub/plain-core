use super::*;
use std::sync::Arc;

#[test]
fn cover_publication_is_content_addressed_and_repairs_incomplete_cache_files() {
    let directory = tempfile::tempdir().unwrap();
    let bytes = include_bytes!("../../../../../../plain-rs/testdata/v-prog-gray.jpg");
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
    let host = Arc::new(crate::infrastructure::host::bridge::Host::default());
    let (_generation, mut requests) = host.connect();
    let resources = crate::infrastructure::host::resources::NativeResources::new(host);
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
        include_bytes!("../../../../../../plain-rs/testdata/v-prog-gray.jpg")
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
    let host = Arc::new(crate::infrastructure::host::bridge::Host::default());
    let (_generation, mut requests) = host.connect();
    let resources = crate::infrastructure::host::resources::NativeResources::new(host);
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
        include_bytes!("../../../../../../plain-rs/testdata/v-prog-gray.jpg")
    );
    assert!(requests.try_recv().is_err());
}

#[tokio::test]
async fn published_cover_is_readable_while_private_cache_neighbors_remain_denied() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::test_runtime::Core::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let source = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/audio-cover.mp3");
    let cover = state.audio.cover(source).await.unwrap().unwrap();
    assert!(
        crate::features::services::files::links::authorize_path(&state, cover.to_str().unwrap())
            .is_ok()
    );
    assert_eq!(
        std::fs::read(&cover).unwrap(),
        include_bytes!("../../../../../../plain-rs/testdata/v-prog-gray.jpg")
    );
    let private = state.cache.root().join("audio-covers/private-neighbor");
    std::fs::create_dir_all(private.parent().unwrap()).unwrap();
    std::fs::write(&private, b"private bytes").unwrap();
    assert!(
        crate::features::services::files::links::authorize_path(&state, private.to_str().unwrap())
            .is_err()
    );
    core.stop_core().await;
}
