use crate::test_runtime::Core as ContentServer;
use serde_json::json;
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn stat_authorizes_the_resolved_path_once_and_rejects_invalid_references_before_host_access()
{
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join("core");
    let server = ContentServer::open(&directory).unwrap();
    let state = server.runtime_state();
    state.file_platform.use_sdk_for_test();
    let path = directory.join("example.txt");
    std::fs::write(&path, b"example").unwrap();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let observed = calls.clone();
    let host = state.host.clone();
    let (generation, mut incoming) = host.connect();
    let worker = tokio::spawn(async move {
        while let Some(request) = incoming.recv().await {
            assert_eq!(request["method"], "systemPermissionFacts");
            observed
                .lock()
                .unwrap()
                .push(request["params"]["permissions"].clone());
            host.reply(
                generation,
                json!({"id":request["id"],"result":{"granted":{"WRITE_EXTERNAL_STORAGE":false}}}),
            )
            .unwrap();
        }
    });
    for reference in [
        "app://example.txt".to_owned(),
        format!("file://{}", path.display()),
    ] {
        assert!(
            plain_rs::services::files::browse::stat_record(&state.file_platform, &reference)
                .await
                .is_some()
        );
    }
    assert!(calls.lock().unwrap().is_empty());
    let external = temp.path().join("external.txt");
    std::fs::write(&external, b"external").unwrap();
    assert!(
        plain_rs::services::files::browse::stat_record(
            &state.file_platform,
            external.to_str().unwrap()
        )
        .await
        .is_none()
    );
    assert_eq!(
        *calls.lock().unwrap(),
        vec![json!(["WRITE_EXTERNAL_STORAGE"])]
    );
    for reference in [
        "",
        " ",
        ".",
        "app://../example.txt",
        "fid:invalid",
        "file://remote/example.txt",
    ] {
        assert!(
            plain_rs::services::files::browse::stat_record(&state.file_platform, reference)
                .await
                .is_none()
        );
    }
    assert_eq!(calls.lock().unwrap().len(), 1);
    server.shutdown().await;
    worker.abort();
}
