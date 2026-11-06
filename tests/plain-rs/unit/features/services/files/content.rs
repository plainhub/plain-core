use super::*;

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
            action: Action::Download,
            size: None,
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
