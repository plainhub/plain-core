use crate::server::ContentServer;

#[tokio::test]
async fn native_import_replaces_sound_in_the_owned_layout_without_host_calls() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let (_generation, mut requests) = state.host.connect();
    let source = directory.path().join("selected+audio.wav");
    let destination = plain_rs::data_paths::DataPaths::new(directory.path()).pomodoro_sound();
    for bytes in [b"first sound".as_slice(), b"replacement sound".as_slice()] {
        std::fs::write(&source, bytes).unwrap();
        let reference = core
            .native_management()
            .unwrap()
            .import_pomodoro_sound(format!("file://{}", source.display()))
            .await
            .unwrap();
        assert_eq!(reference, "app://audio/pomodoro_sound.mp3");
        assert_eq!(std::fs::read(&destination).unwrap(), bytes);
        assert!(source.exists());
        assert_eq!(
            std::fs::read_dir(destination.parent().unwrap())
                .unwrap()
                .count(),
            1
        );
    }
    assert!(requests.try_recv().is_err());
    let response = crate::graphql::executor::execute_graphql(
        &state.public,
        async_graphql::Request::new(r#"mutation{importPomodoroSound(source:"/missing")}"#),
        crate::graphql::executor::RequestContext::remote(Some(state.clone())),
    )
    .await;
    assert!(!response.errors.is_empty());
    assert_eq!(std::fs::read(&destination).unwrap(), b"replacement sound");
    core.stop_core().await;
}
