use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

struct Waiting(Arc<AtomicBool>);

#[cfg(feature = "media")]
fn filesystem_media_host(
    system_catalog: bool,
) -> (
    tempfile::TempDir,
    crate::server::ContentServer,
    std::path::PathBuf,
) {
    let directory = tempfile::Builder::new()
        .prefix("media-host-start-")
        .tempdir_in(plain_rs::infrastructure::platform::system::consts::home_directory().unwrap())
        .unwrap();
    let data = directory.path().join("data");
    let cache = directory.path().join("cache");
    let sources = directory.path().join("sources");
    std::fs::create_dir_all(&sources).unwrap();
    std::fs::write(sources.join("owned.txt"), b"owned source").unwrap();
    std::fs::write(
        sources.join("owned.mp3"),
        include_bytes!("../../../../plain-rs/testdata/audio-untagged.mp3"),
    )
    .unwrap();
    std::fs::create_dir_all(&data).unwrap();
    let prefs = Arc::new(
        plain_rs::features::prefs::Prefs::load(&plain_rs::features::prefs::default_path(&data))
            .unwrap(),
    );
    plain_rs::features::prefs::identity::ensure_url_token(&prefs);
    let mut config = plain_rs::infrastructure::platform::startup::config();
    config.cache_dir = Some(cache.to_string_lossy().into_owned());
    config.system_media_catalog = system_catalog;
    let core = crate::server::ContentServer::start_core_with_config(
        &data,
        &plain_libs::crypto::gen_token(),
        prefs,
        config,
    )
    .unwrap();
    core.runtime_state()
        .media_scans
        .set_defaults_for_test(plain_rs::features::prefs::file_index::Defaults {
            scan_directories: Vec::new(),
            excluded_directories: Vec::new(),
            excluded_names: Vec::new(),
        })
        .unwrap();
    core.shared_preferences()
        .unwrap()
        .set(
            plain_rs::features::prefs::pref_keys::MEDIA_SOURCE_DIRS,
            vec![sources.to_str().unwrap()],
        )
        .unwrap();
    (directory, core, sources)
}

#[cfg(feature = "media")]
#[tokio::test]
async fn connecting_a_filesystem_media_host_indexes_configured_sources_without_host_scan_calls() {
    let _discovery = plain_rs::features::services::discovery::mdns_runtime::TEST_LOCK
        .lock()
        .await;
    let (_directory, core, sources) = filesystem_media_host(false);
    let session = Arc::new(core.host_events().unwrap());
    let runner = tokio::spawn(session.serve(|method, _params| async move {
        match method.as_str() {
            "systemCapabilities" => Ok(json!(["MEDIA_SCAN", "PEER_DISCOVERY"])),
            _ => Ok(json!(true)),
        }
    }));
    let catalog = core.native_media_catalog().unwrap();
    let indexed = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if catalog
                .count(plain_rs::features::enums::DataType::Doc)
                .unwrap()
                == 1
                && catalog
                    .count(plain_rs::features::enums::DataType::Audio)
                    .unwrap()
                    == 1
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await;
    let source = std::fs::read(sources.join("owned.txt")).unwrap();
    core.runtime_state()
        .media_scans
        .wait_background()
        .await
        .unwrap();
    let initial_scan = catalog.file_scan_status().unwrap().unwrap().id;
    let replacement = Arc::new(core.host_events().unwrap());
    runner.await.unwrap().unwrap();
    let runner = tokio::spawn(replacement.serve(|method, _params| async move {
        match method.as_str() {
            "systemCapabilities" => Ok(json!(["MEDIA_SCAN", "PEER_DISCOVERY"])),
            _ => Ok(json!(true)),
        }
    }));
    plain_rs::features::services::files::catalog_scan::host_connected(
        core.runtime().clone(),
        core.runtime_state().host.generation().unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(
        catalog.file_scan_status().unwrap().unwrap().id,
        initial_scan
    );
    assert_eq!(
        std::fs::read(sources.join("owned.mp3")).unwrap(),
        include_bytes!("../../../../plain-rs/testdata/audio-untagged.mp3")
    );
    core.stop_core().await;
    runner.await.unwrap().unwrap();
    assert!(
        indexed.is_ok(),
        "Host startup did not index its configured source"
    );
    assert_eq!(source, b"owned source");
}

#[cfg(feature = "media")]
#[tokio::test]
async fn hosts_without_media_scan_capability_do_not_initialize_filesystem_indexing() {
    let (_directory, core, _sources) = filesystem_media_host(false);
    let session = Arc::new(core.host_events().unwrap());
    let runner = tokio::spawn(session.serve(|method, _params| async move {
        Ok(if method == "systemCapabilities" {
            json!([])
        } else {
            json!(true)
        })
    }));
    plain_rs::features::services::files::catalog_scan::host_connected(
        core.runtime().clone(),
        core.runtime_state().host.generation().unwrap(),
    )
    .await
    .unwrap();
    assert!(
        core.runtime_state()
            .media_scans
            .policy()
            .unwrap()
            .scope
            .is_none()
    );
    assert!(
        core.runtime_state()
            .media_scans
            .snapshot()
            .unwrap()
            .is_none()
    );
    core.stop_core().await;
    runner.await.unwrap().unwrap();
}

#[cfg(feature = "media")]
#[tokio::test]
async fn system_catalog_hosts_do_not_initialize_filesystem_indexing_or_request_capabilities() {
    let (_directory, core, _sources) = filesystem_media_host(true);
    plain_rs::features::services::files::catalog_scan::host_connected(core.runtime().clone(), 0)
        .await
        .unwrap();
    assert!(
        core.runtime_state()
            .media_scans
            .policy()
            .unwrap()
            .scope
            .is_none()
    );
    assert!(
        core.runtime_state()
            .media_scans
            .snapshot()
            .unwrap()
            .is_none()
    );
    core.stop_core().await;
}

impl Drop for Waiting {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn serving_handles_callbacks_while_request_waits_and_cancels_on_stop() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let session = Arc::new(core.host_events().unwrap());
    let dropped = Arc::new(AtomicBool::new(false));
    let started = Arc::new(tokio::sync::Notify::new());
    let (events, mut received) = tokio::sync::mpsc::unbounded_channel();
    let runner = tokio::spawn(session.serve({
        let dropped = dropped.clone();
        let started = started.clone();
        move |method, _params| {
            let dropped = dropped.clone();
            let started = started.clone();
            let events = events.clone();
            async move {
                if method == "systemWebLoginRequested" {
                    events.send(method.clone()).unwrap();
                }
                if method == "waitingRequest" {
                    let _waiting = Waiting(dropped);
                    started.notify_one();
                    std::future::pending::<()>().await;
                }
                Ok(json!(true))
            }
        }
    }));
    let host = core.runtime_state().host.clone();
    let call = tokio::spawn(async move { host.call("waitingRequest", json!({})).await });
    started.notified().await;
    core.runtime_state()
        .host
        .notify("systemWebLoginRequested", json!({}))
        .unwrap();
    assert_eq!(received.recv().await.unwrap(), "systemWebLoginRequested");
    assert!(!dropped.load(Ordering::SeqCst));
    core.stop_core().await;
    runner.await.unwrap().unwrap();
    assert!(dropped.load(Ordering::SeqCst));
    assert!(call.await.unwrap().is_err());
}

#[tokio::test]
async fn serving_preserves_request_errors_and_old_session_cannot_disconnect_replacement() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let session = Arc::new(core.host_events().unwrap());
    let runner = tokio::spawn(session.serve(|method, _params| async move {
        if method == "failingRequest" {
            Err("platform failure".to_string())
        } else {
            Ok(json!(true))
        }
    }));
    assert_eq!(
        core.runtime_state()
            .host
            .call("failingRequest", json!({}))
            .await
            .unwrap_err(),
        "platform failure"
    );
    let replacement = core.host_events().unwrap();
    runner.await.unwrap().unwrap();
    let host = core.runtime_state().host.clone();
    let request = tokio::spawn(async move { host.call("replacementRequest", json!({})).await });
    loop {
        let next = replacement.next_request().await.unwrap();
        replacement
            .reply(json!({"id": next["id"], "result": true}))
            .unwrap();
        if next["method"] == "replacementRequest" {
            break;
        }
    }
    assert_eq!(request.await.unwrap().unwrap(), json!(true));
    core.stop_core().await;
}

#[tokio::test]
async fn aborting_runner_disconnects_session_even_with_retained_handle() {
    let directory = tempfile::tempdir().unwrap();
    let core = crate::server::ContentServer::open(directory.path()).unwrap();
    let session = Arc::new(core.host_events().unwrap());
    let started = Arc::new(tokio::sync::Notify::new());
    let runner = tokio::spawn(session.clone().serve({
        let started = started.clone();
        move |method, _params| {
            let started = started.clone();
            async move {
                if method == "waitingRequest" {
                    started.notify_one();
                    std::future::pending::<()>().await;
                }
                Ok(json!(true))
            }
        }
    }));
    let host = core.runtime_state().host.clone();
    let call = tokio::spawn(async move { host.call("waitingRequest", json!({})).await });
    started.notified().await;
    runner.abort();
    assert!(runner.await.unwrap_err().is_cancelled());
    assert!(core.runtime_state().host.generation().is_none());
    assert!(call.await.unwrap().is_err());
    drop(session);
    core.stop_core().await;
}
