use crate::server::ContentServer;

#[tokio::test]
async fn native_import_preserves_image_bytes_and_resolves_duplicate_names_in_rust() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let source = directory.path().join("source.png");
    std::fs::write(&source, b"image bytes").unwrap();
    let (_generation, mut requests) = state.host.connect();
    for name in ["picture.png", "picture (1).png"] {
        let reference = core
            .native_management()
            .unwrap()
            .import_note_image(source.to_string_lossy().into_owned(), "picture.png".into())
            .await
            .unwrap();
        assert_eq!(reference, format!("app://note-images/{name}"));
        assert_eq!(
            std::fs::read(
                plain_rs::data_paths::DataPaths::new(directory.path())
                    .note_images()
                    .join(name)
            )
            .unwrap(),
            b"image bytes"
        );
    }
    assert!(source.exists());
    assert!(requests.try_recv().is_err());
    let remote = crate::graphql::executor::execute_graphql(
        &state.public,
        async_graphql::Request::new(
            r#"mutation{importNoteImage(source:"/missing",fileName:"remote.png")}"#,
        ),
        crate::graphql::executor::RequestContext::remote(Some(state.clone())),
    )
    .await;
    assert!(!remote.errors.is_empty());
    core.stop_core().await;
}
