use crate::server::ContentServer;

#[tokio::test]
async fn native_note_image_import_uses_shared_app_file_storage() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let source = directory.path().join("source.png");
    std::fs::write(&source, b"image bytes").unwrap();
    let (_generation, mut requests) = state.host.connect();
    let mut imported = Vec::new();
    for _ in 0..2 {
        let reference = core
            .native_management()
            .unwrap()
            .import_note_image(source.to_string_lossy().into_owned(), "picture.png".into())
            .await
            .unwrap();
        assert!(reference.starts_with("fid:"));
        let path = plain_rs::features::services::files::references::resolve(
            state.db.clone(),
            state.directory.clone(),
            &reference,
        )
        .unwrap();
        assert!(std::path::Path::new(&path).starts_with(directory.path().join("files")));
        assert_eq!(std::fs::read(path).unwrap(), b"image bytes");
        imported.push(reference);
    }
    assert_eq!(imported[0], imported[1]);
    assert_eq!(state.db.count_app_files(), 1);
    assert!(!directory.path().join("note-images").exists());
    assert!(source.exists());
    assert!(requests.try_recv().is_err());
    let remote = crate::graphql::executor::execute_graphql(
        &state.public,
        plain_graphql::Request::new(
            r#"mutation{importNoteImage(source:"/missing",fileName:"remote.png")}"#,
        ),
        crate::graphql::executor::RequestContext::remote(Some(state.clone())),
    )
    .await;
    assert!(!remote.errors.is_empty());
    core.stop_core().await;
}
