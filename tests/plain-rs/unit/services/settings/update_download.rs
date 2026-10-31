use super::*;
use crate::test_runtime::Core as ContentServer;
use anyhow::anyhow;
use serde_json::json;
use std::sync::Arc;

#[test]
fn progress_is_bounded_without_overflow_and_unknown_lengths_do_not_invent_progress() {
    assert_eq!(percentage(u64::MAX, Some(u64::MAX)), 99);
    assert_eq!(percentage(u64::MAX, Some(1)), 99);
    assert_eq!(percentage(50, Some(100)), 50);
    assert_eq!(percentage(50, Some(0)), 0);
    assert_eq!(percentage(50, None), 0);
    for raw in [
        "http://example.invalid/update",
        "file:///tmp/update",
        "https://user:secret@localhost/update",
    ] {
        assert!(validate_url(raw).is_err());
    }
}

fn setup() -> (tempfile::TempDir, ContentServer, CoreRuntime, Identity) {
    let dir = tempfile::tempdir().unwrap();
    let core = ContentServer::open_with_cache(&dir.path().join("data"), &dir.path().join("cache"))
        .unwrap();
    let state = core.runtime_state();
    updates::update(
        &state,
        json!({"newVersion":"999.0.0","downloadUrl":"https://example.invalid/update","size":4}),
    )
    .unwrap();
    let identity = Identity::from(&plain_rs::services::settings::updates::info(&state).unwrap());
    (dir, core, state, identity)
}

fn allow_updates(state: &CoreRuntime) -> tokio::sync::mpsc::UnboundedReceiver<serde_json::Value> {
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
async fn truncated_or_failed_streams_remove_staging_and_never_publish_a_download_path() {
    let (_dir, core, state, identity) = setup();
    let (_cancel, mut receiver) = watch::channel(false);
    for chunks in [
        vec![Ok(&b"ab"[..])],
        vec![Ok(&b"abcdx"[..])],
        vec![Err(anyhow!("broken body"))],
        vec![],
    ] {
        assert!(
            save_stream(
                &state,
                "failed",
                &identity,
                futures_util::stream::iter(chunks),
                Some(4),
                &mut receiver
            )
            .await
            .is_err()
        );
        assert_eq!(
            std::fs::read_dir(state.cache.app_updates())
                .unwrap()
                .count(),
            0
        );
        assert!(
            plain_rs::services::settings::updates::info(&state)
                .unwrap()
                .downloaded_apk_path
                .is_empty()
        );
    }
    assert!(
        save_stream(
            &state,
            "wrong-length",
            &identity,
            futures_util::stream::iter([Ok(&b"abcd"[..])]),
            Some(5),
            &mut receiver
        )
        .await
        .is_err()
    );
    core.stop_core().await;
}

#[tokio::test]
async fn cancellation_interrupts_a_pending_body_and_cleans_staging_before_join_returns() {
    let (_dir, core, state, identity) = setup();
    let (cancel, mut receiver) = watch::channel(false);
    let (entered, waiting) = tokio::sync::oneshot::channel();
    let mut entered = Some(entered);
    let body = futures_util::stream::poll_fn(move |_| {
        if let Some(entered) = entered.take() {
            let _ = entered.send(());
        }
        std::task::Poll::<Option<Result<&'static [u8]>>>::Pending
    });
    let worker_state = state.clone();
    let worker = tokio::spawn(async move {
        save_stream(
            &worker_state,
            "cancelled",
            &identity,
            body,
            None,
            &mut receiver,
        )
        .await
    });
    waiting.await.unwrap();
    cancel.send_replace(true);
    assert!(worker.await.unwrap().is_err());
    assert_eq!(
        std::fs::read_dir(state.cache.app_updates())
            .unwrap()
            .count(),
        0
    );
    core.stop_core().await;
}

#[tokio::test]
async fn changed_release_or_missing_artifact_cannot_be_reported_as_completed() {
    let (_dir, core, state, identity) = setup();
    allow_updates(&state);
    let (_cancel, mut receiver) = watch::channel(false);
    updates::update(
        &state,
        json!({"downloadUrl":"https://example.invalid/replaced"}),
    )
    .unwrap();
    assert!(
        save_stream(
            &state,
            "stale",
            &identity,
            futures_util::stream::iter([Ok(&b"abcd"[..])]),
            Some(4),
            &mut receiver
        )
        .await
        .is_err()
    );
    assert_eq!(
        std::fs::read_dir(state.cache.app_updates())
            .unwrap()
            .count(),
        0
    );
    updates::update(&state, json!({"downloadUrl":identity.url})).unwrap();
    let path = save_stream(
        &state,
        "removed",
        &identity,
        futures_util::stream::iter([Ok(&b"abcd"[..])]),
        Some(4),
        &mut receiver,
    )
    .await
    .unwrap();
    std::fs::remove_file(path).unwrap();
    core.stop_core().await;
    let core =
        ContentServer::open_with_cache(&_dir.path().join("data"), &_dir.path().join("cache"))
            .unwrap();
    let state = core.runtime_state();
    let snapshot = execute_owned(state.clone(), Command::Reconcile)
        .await
        .unwrap();
    assert_ne!(snapshot.status, AppUpdateDownloadStatus::Completed);
    assert!(
        plain_rs::services::settings::updates::info(&state)
            .unwrap()
            .downloaded_apk_path
            .is_empty()
    );
    core.stop_core().await;
}

#[tokio::test]
async fn cancel_command_awaits_worker_cleanup_and_shutdown_rejects_later_commands() {
    let (_dir, core, state, _) = setup();
    let runtime = state.update_checks.download.clone();
    let (cancel, mut receiver) = watch::channel(false);
    let cleaned = Arc::new(AtomicBool::new(false));
    let completed = cleaned.clone();
    *runtime.command.lock().await = Some(Task {
        cancel,
        join: tokio::spawn(async move {
            receiver.wait_for(|cancel| *cancel).await.unwrap();
            tokio::task::yield_now().await;
            completed.store(true, Ordering::Release);
        }),
    });
    execute_owned(state.clone(), Command::Cancel).await.unwrap();
    assert!(cleaned.load(Ordering::Acquire));
    runtime.shutdown().await;
    assert!(execute_owned(state.clone(), Command::Start).await.is_err());
    core.stop_core().await;
}

#[tokio::test]
async fn settings_changes_cancel_the_current_transfer_and_clear_its_persisted_artifact() {
    let (_dir, core, state, _) = setup();
    let (cancel, receiver) = watch::channel(false);
    state.update_checks.download.state.lock().unwrap().cancel = Some(cancel);
    updates::update(&state, json!({"downloadedApkPath":"/not-owned/package"})).unwrap();
    assert!(
        updates::set_setting(&state, "skip_version", json!("999.0.0"))
            .await
            .unwrap()
    );
    assert!(*receiver.borrow());
    assert!(
        plain_rs::services::settings::updates::info(&state)
            .unwrap()
            .downloaded_apk_path
            .is_empty()
    );
    core.stop_core().await;
}

#[tokio::test]
async fn dropping_the_request_does_not_detach_worker_cleanup_from_the_serialized_command() {
    let (_dir, core, state, _) = setup();
    let (cancel, mut receiver) = watch::channel(false);
    let (entered, waiting) = tokio::sync::oneshot::channel();
    let (release, released) = tokio::sync::oneshot::channel();
    let cleaned = Arc::new(AtomicBool::new(false));
    let completed = cleaned.clone();
    *state.update_checks.download.command.lock().await = Some(Task {
        cancel,
        join: tokio::spawn(async move {
            receiver.wait_for(|cancel| *cancel).await.unwrap();
            let _ = entered.send(());
            released.await.unwrap();
            completed.store(true, Ordering::Release);
        }),
    });
    let (generation, mut requests) = crate::test_support::HostRequests::connect(&state.host);
    let host = state.host.clone();
    let capabilities = tokio::spawn(async move {
        let request = requests.recv().await.unwrap();
        assert_eq!(request["method"], "systemCapabilities");
        host.reply(
            generation,
            json!({"id":request["id"],"result":["APP_UPDATES"]}),
        )
        .unwrap();
        requests
    });
    let worker_state = state.clone();
    let caller = tokio::spawn(async move { execute(&worker_state, Command::Cancel).await });
    waiting.await.unwrap();
    caller.abort();
    let _ = caller.await;
    assert!(!cleaned.load(Ordering::Acquire));
    release.send(()).unwrap();
    let snapshot = execute_owned(state.clone(), Command::Reconcile)
        .await
        .unwrap();
    assert_eq!(snapshot.status, AppUpdateDownloadStatus::Idle);
    assert!(cleaned.load(Ordering::Acquire));
    assert!(capabilities.await.unwrap().try_recv().is_err());
    core.stop_core().await;
}

#[tokio::test]
async fn clearing_application_cache_waits_for_the_active_body_before_deleting_its_directory() {
    let (_dir, core, state, identity) = setup();
    let (cancel, mut receiver) = watch::channel(false);
    let (entered, waiting) = tokio::sync::oneshot::channel();
    let mut entered = Some(entered);
    let body = futures_util::stream::poll_fn(move |_| {
        if let Some(entered) = entered.take() {
            let _ = entered.send(());
        }
        std::task::Poll::<Option<Result<&'static [u8]>>>::Pending
    });
    let cleaned = Arc::new(AtomicBool::new(false));
    let completed = cleaned.clone();
    let worker_state = state.clone();
    let join = tokio::spawn(async move {
        assert!(
            save_stream(
                &worker_state,
                "cache-clear",
                &identity,
                body,
                None,
                &mut receiver
            )
            .await
            .is_err()
        );
        completed.store(true, Ordering::Release);
    });
    *state.update_checks.download.command.lock().await = Some(Task { cancel, join });
    waiting.await.unwrap();
    plain_rs::services::cache::clear_application(&state)
        .await
        .unwrap();
    assert!(cleaned.load(Ordering::Acquire));
    assert!(!state.cache.app_updates().exists());
    assert_eq!(
        state
            .update_checks
            .download
            .state
            .lock()
            .unwrap()
            .snapshot
            .status,
        AppUpdateDownloadStatus::Idle
    );
    core.stop_core().await;
}
