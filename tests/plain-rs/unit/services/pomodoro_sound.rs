use super::*;
use crate::test_runtime::Core as ContentServer;
use serde_json::json;

#[tokio::test]
async fn interrupted_platform_read_preserves_the_previous_sound_and_removes_temporary_files() {
    for outcome in ["failure", "cancel", "finish"] {
        let directory = tempfile::tempdir().unwrap();
        let core = ContentServer::open(directory.path()).unwrap();
        let state = core.runtime_state();
        let destination = plain_rs::data_paths::DataPaths::new(directory.path()).pomodoro_sound();
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::write(&destination, b"old sound").unwrap();
        let (generation, mut requests) = state.host.connect();
        let task = {
            let state = state.clone();
            tokio::spawn(async move { import(&state, "content://fixture/sound").await })
        };
        let request = requests.recv().await.unwrap();
        assert_eq!(request["method"], "fileResourceStream");
        let stream = state
            .resources
            .attach_native(request["params"]["id"].as_str().unwrap(), generation)
            .unwrap();
        state
            .host
            .reply(generation, json!({"id":request["id"],"result":true}))
            .unwrap();
        stream.write(b"new sound".to_vec()).await.unwrap();
        match outcome {
            "failure" => stream.fail().await.unwrap(),
            "finish" => stream.finish().await.unwrap(),
            _ => core.stop_core().await,
        }
        let result = task.await.unwrap();
        let expected = if outcome == "finish" {
            assert!(result.is_ok());
            b"new sound"
        } else {
            assert!(result.is_err());
            b"old sound"
        };
        assert_eq!(std::fs::read(&destination).unwrap(), expected);
        assert_eq!(
            std::fs::read_dir(destination.parent().unwrap())
                .unwrap()
                .count(),
            1
        );
        core.stop_core().await;
    }
}

#[tokio::test]
async fn invalid_or_stopped_sources_do_not_replace_sound() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let source = directory.path().join("selected");
    assert!(import(&state, source.to_str().unwrap()).await.is_err());
    assert!(
        import(&state, directory.path().to_str().unwrap())
            .await
            .is_err()
    );
    core.stop_core().await;
    std::fs::write(&source, b"stopped").unwrap();
    assert!(import(&state, source.to_str().unwrap()).await.is_err());
    assert!(
        !plain_rs::data_paths::DataPaths::new(directory.path())
            .pomodoro_sound()
            .exists()
    );
}
