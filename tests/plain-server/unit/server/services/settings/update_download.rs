use crate::server::ContentServer;
use crate::server::ServerState;
use plain_rs::services::settings::update_download::*;
use plain_rs::services::settings::updates;
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

fn setup() -> (tempfile::TempDir, ContentServer, ServerState) {
    let dir = tempfile::tempdir().unwrap();
    let core = ContentServer::open_with_cache(&dir.path().join("data"), &dir.path().join("cache"))
        .unwrap();
    let state = core.runtime_state();
    updates::update(
        &state,
        json!({"newVersion":"999.0.0","downloadUrl":"https://example.invalid/update","size":4}),
    )
    .unwrap();
    (dir, core, state)
}

fn allow_updates(state: &ServerState) -> tokio::sync::mpsc::UnboundedReceiver<serde_json::Value> {
    let host = state.host.clone();
    let (generation, mut requests) = host.connect();
    let (sent, received) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Some(request) = requests.recv().await {
            let result = match request["method"].as_str().unwrap() {
                "systemCapabilities" => json!(["APP_UPDATES"]),
                "appUpdateDownloadChanged" => {
                    let _ = sent.send(request["params"].clone());
                    json!(null)
                }
                other => panic!("unexpected host method: {other}"),
            };
            host.reply(generation, json!({"id":request["id"],"result":result}))
                .unwrap();
        }
    });
    received
}

#[tokio::test]
async fn complete_stream_is_published_with_identity_and_restored_without_client_file_checks() {
    let (_dir, core, state) = setup();
    let mut reports = allow_updates(&state);
    let (_cancel, _receiver) = watch::channel(false);
    let mut events = state.events.subscribe();
    let path = plain_rs::services::settings::update_download::test_fixture::save(
        &state,
        "complete",
        vec![b"a".to_vec(), b"b".to_vec(), b"cd".to_vec()],
    )
    .await
    .unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"abcd");
    assert_eq!(
        Path::new(&path).parent().unwrap().parent().unwrap(),
        state.cache.app_updates()
    );
    let _info = plain_rs::services::settings::updates::info(&state).unwrap();
    let mut progress = Vec::new();
    for _ in 0..3 {
        let report = tokio::time::timeout(Duration::from_secs(3), reports.recv())
            .await
            .unwrap()
            .unwrap();
        progress.push(report["progress"].as_i64().unwrap());
    }
    assert_eq!(progress, vec![25, 50, 99]);
    while let Ok(event) = events.try_recv() {
        assert_ne!(event.event_type, "APP_UPDATE_DOWNLOAD_UPDATED");
    }
    core.stop_core().await;
    let core =
        ContentServer::open_with_cache(&_dir.path().join("data"), &_dir.path().join("cache"))
            .unwrap();
    let state = core.runtime_state();
    let bridge = Arc::new(core.host_events().unwrap());
    let (sent, mut reports) = tokio::sync::mpsc::unbounded_channel();
    let serving = tokio::spawn(bridge.serve(move |method, params| {
        let sent = sent.clone();
        async move {
            match method.as_str() {
                "systemCapabilities" => Ok(json!(["APP_UPDATES"])),
                "appUpdateDownloadChanged" => {
                    let _ = sent.send(params);
                    Ok(json!(null))
                }
                _ => Ok(json!(null)),
            }
        }
    }));
    let snapshot = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let snapshot = reports.recv().await.unwrap();
            if snapshot["status"] == "COMPLETED" {
                break snapshot;
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(snapshot["filePath"], path);
    let cancelled = execute(&state, Command::Cancel).await.unwrap();
    assert_eq!(cancelled.status, AppUpdateDownloadStatus::Idle);
    assert!(!Path::new(&path).exists());
    core.stop_core().await;
    serving.await.unwrap().unwrap();
}

#[tokio::test]
async fn native_download_command_requires_current_capability_before_starting_network_work() {
    let (_dir, core, state) = setup();
    let (generation, mut requests) = state.host.connect();
    let host = state.host.clone();
    let reply = tokio::spawn(async move {
        let request = requests.recv().await.unwrap();
        assert_eq!(request["method"], "systemCapabilities");
        host.reply(generation, json!({"id":request["id"],"result":[]}))
            .unwrap();
        requests
    });
    let response = core
        .native_management()
        .unwrap()
        .start_app_update_download()
        .await;
    assert!(
        response
            .err()
            .unwrap()
            .contains("unsupported capability APP_UPDATES")
    );
    let retired = core
        .native_graphql()
        .unwrap()
        .execute(async_graphql::Request::new(
            "{ appUpdateDownload { status } }",
        ))
        .await;
    assert!(!retired.errors.is_empty());
    assert!(reply.await.unwrap().try_recv().is_err());
    assert!(!state.cache.app_updates().exists());
    let remote = crate::graphql::executor::execute_graphql(
        &state.public,
        async_graphql::Request::new("mutation { cancelAppUpdateDownload { status } }"),
        crate::graphql::executor::RequestContext::remote(Some(state.clone())),
    )
    .await;
    assert!(!remote.errors.is_empty());
    core.stop_core().await;
}
