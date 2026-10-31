use super::*;
use serde_json::json;
use std::sync::Arc;

#[test]
fn default_shared_download_layout_uses_startup_root_without_another_host_call() {
    let root = tempfile::tempdir().unwrap();
    let host =
        plain_rs::host::bridge::Host::with_startup_config(plain_rs::host::startup::StartupConfig {
            downloads_dir: root.path().to_str().unwrap().into(),
            ..plain_rs::platform::startup::config()
        })
        .unwrap();
    let (_generation, mut requests) = host.connect();
    for _ in 0..3 {
        assert_eq!(
            downloads_directory(&host, "").unwrap(),
            root.path().join("PlainApp").to_str().unwrap()
        );
    }
    let selected = root.path().join("selected");
    assert_eq!(
        downloads_directory(&host, selected.to_str().unwrap()).unwrap(),
        selected.to_str().unwrap()
    );
    assert!(requests.try_recv().is_err());
    assert!(!root.path().join("PlainApp").exists());
}

#[test]
fn invalid_default_download_roots_fail_without_replacing_an_explicit_target() {
    let root = tempfile::tempdir().unwrap();
    for value in ["", "relative", "bad\0path"] {
        let host = plain_rs::host::bridge::Host::with_startup_config(
            plain_rs::host::startup::StartupConfig {
                downloads_dir: value.into(),
                ..plain_rs::platform::startup::config()
            },
        )
        .unwrap();
        assert!(downloads_directory(&host, "").is_err());
        assert_eq!(
            downloads_directory(&host, root.path().to_str().unwrap()).unwrap(),
            root.path().to_str().unwrap()
        );
        assert!(downloads_directory(&host, "relative").is_err());
    }
}
fn fixture() -> (tempfile::TempDir, crate::test_runtime::Core, CoreRuntime) {
    let directory = tempfile::tempdir().unwrap();
    let prefs =
        Arc::new(plain_rs::prefs::Prefs::load(&directory.path().join("prefs.json")).unwrap());
    prefs.set("client_id", "fixture").unwrap();
    let server = crate::test_runtime::Core::start(
        directory.path(),
        &plain_rs::utils::base64::base64_encode(&[3; 32]),
        prefs,
    )
    .unwrap();
    let state = server.runtime_state();
    (directory, server, state)
}

#[tokio::test]
async fn shared_download_changes_refresh_the_host_without_a_broadcast_event() {
    let (_directory, server, state) = fixture();
    let (generation, mut requests) = state.host.connect();
    let mut events = state.events.subscribe();
    state.shared_batches.queue.stop();
    let request = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let request = requests.recv().await.unwrap();
            state
                .host
                .reply(generation, json!({"id":request["id"],"result":null}))
                .unwrap();
            if request["method"] == "sharedFolderDownloadsRefresh" {
                break request;
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(request["params"], json!({}));
    while let Ok(event) = events.try_recv() {
        assert_ne!(event.event_type, "SHARED_FOLDER_DOWNLOAD_UPDATED");
    }
    state.host.disconnect(generation);
    server.shutdown().await;
}
