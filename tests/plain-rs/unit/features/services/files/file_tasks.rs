use super::*;
use crate::test_runtime::Core as ContentServer;
use serde_json::json;

#[tokio::test(start_paused = true)]
async fn synchronous_completion_waits_for_notification_without_periodic_reads() {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    let notification = Arc::new(tokio::sync::Notify::new());
    let ready = Arc::new(AtomicBool::new(false));
    let reads = Arc::new(AtomicUsize::new(0));
    let (started, pending) = tokio::sync::oneshot::channel();
    let worker = {
        let notification = notification.clone();
        let ready = ready.clone();
        let reads = reads.clone();
        tokio::spawn(async move {
            let mut started = Some(started);
            wait_for_completion(&notification, || {
                reads.fetch_add(1, Ordering::SeqCst);
                if let Some(started) = started.take() {
                    started.send(()).unwrap();
                }
                Ok(ready.load(Ordering::SeqCst).then_some(7))
            })
            .await
        })
    };
    pending.await.unwrap();
    tokio::time::advance(std::time::Duration::from_secs(10)).await;
    tokio::task::yield_now().await;
    assert_eq!(reads.load(Ordering::SeqCst), 1);
    ready.store(true, Ordering::SeqCst);
    notification.notify_waiters();
    assert_eq!(worker.await.unwrap().unwrap(), 7);
    assert_eq!(reads.load(Ordering::SeqCst), 2);
}

#[tokio::test(start_paused = true)]
async fn completion_between_state_read_and_wait_is_not_lost() {
    let notification = tokio::sync::Notify::new();
    let mut reads = 0;
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        wait_for_completion(&notification, || {
            reads += 1;
            if reads == 1 {
                notification.notify_waiters();
                Ok(None)
            } else {
                Ok(Some("done"))
            }
        }),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result, "done");
    assert_eq!(reads, 2);
}

#[tokio::test]
async fn destructive_path_batches_validate_every_target_before_touching_any_file_or_host() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    state.file_platform.use_sdk_for_test();
    let (_generation, mut requests) = state.host.connect();
    let path = directory.path().join("preserved.txt");
    std::fs::write(&path, b"preserve me").unwrap();
    for invalid in ["", " \t\n", "/", "."] {
        let paths = vec![path.to_string_lossy().into_owned(), invalid.into()];
        assert!(state.files.trash_paths(paths.clone()).await.is_err());
        assert!(state.files.delete_many(paths).await.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"preserve me");
        assert!(requests.try_recv().is_err());
    }
    for suffix in [".", ".."] {
        let invalid = directory.path().join(suffix).to_string_lossy().into_owned();
        let paths = vec![path.to_string_lossy().into_owned(), invalid];
        assert!(state.files.trash_paths(paths.clone()).await.is_err());
        assert!(state.files.delete_many(paths).await.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"preserve me");
        assert!(requests.try_recv().is_err());
    }
    assert_eq!(state.files.trash_paths(vec![]).await.unwrap(), 0);
    assert_eq!(state.files.delete_many(vec![]).await.unwrap(), 0);
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}

async fn whole_selection_preflight_case(sdk: bool) {
    for kind in [FileTaskType::Copy, FileTaskType::Move] {
        let directory = tempfile::tempdir().unwrap();
        let core = ContentServer::open(&directory.path().join("core")).unwrap();
        let state = core.runtime_state();
        if sdk {
            state.file_platform.use_sdk_for_test();
        }
        let (generation, mut requests) = state.host.connect();
        let host = state.host.clone();
        let receipts = Arc::new(Mutex::new(Vec::<String>::new()));
        let observed = receipts.clone();
        let worker = tokio::spawn(async move {
            while let Some(request) = requests.recv().await {
                let method = request["method"].as_str().unwrap().to_owned();
                observed.lock().unwrap().push(method.clone());
                host.reply(generation, json!({"id":request["id"],"result":if method == "systemPermissionFacts" { json!({"granted":{"WRITE_EXTERNAL_STORAGE":true}}) } else { json!(true) }})).unwrap();
            }
        });
        let source = directory.path().join("selected-first.bin");
        let destination = directory.path().join("first-result.bin");
        let missing = directory.path().join("selected-second-missing.bin");
        let missing_destination = directory.path().join("second-result.bin");
        std::fs::write(&source, b"first selected exact bytes").unwrap();
        let client = state.native_client_id().unwrap();
        let before = state.files.list(client.clone()).await.unwrap();
        let result = state
            .files
            .create(
                &client,
                kind,
                "whole-selection",
                vec![
                    FileTaskOp {
                        src: source.to_string_lossy().into_owned(),
                        dst: destination.to_string_lossy().into_owned(),
                        overwrite: false,
                    },
                    FileTaskOp {
                        src: missing.to_string_lossy().into_owned(),
                        dst: missing_destination.to_string_lossy().into_owned(),
                        overwrite: false,
                    },
                ],
            )
            .await;
        assert!(
            result.is_err(),
            "a missing selected source was admitted: {result:?}"
        );
        assert_eq!(
            result
                .unwrap_err()
                .downcast_ref::<std::io::Error>()
                .unwrap()
                .kind(),
            std::io::ErrorKind::NotFound
        );
        assert_eq!(
            serde_json::to_value(state.files.list(client).await.unwrap()).unwrap(),
            serde_json::to_value(before).unwrap()
        );
        assert_eq!(
            std::fs::read(&source).unwrap(),
            b"first selected exact bytes"
        );
        assert!(!destination.exists());
        assert!(!missing_destination.exists());
        let calls = receipts.lock().unwrap().clone();
        if sdk {
            assert_eq!(calls, vec!["systemPermissionFacts"]);
        } else {
            assert!(calls.is_empty());
        }
        worker.abort();
        let _ = worker.await;
        core.shutdown().await;
    }
}

#[tokio::test]
async fn sdk_admission_checks_every_selected_source_before_recording_a_task_or_running_the_first_operation()
 {
    whole_selection_preflight_case(true).await;
}

#[tokio::test]
async fn native_admission_keeps_the_same_whole_selection_contract_for_copy_and_move() {
    whole_selection_preflight_case(false).await;
}
