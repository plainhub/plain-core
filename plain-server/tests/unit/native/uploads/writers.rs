use super::*;
use plain_rs::services::files::upload_chunks::UploadChunks;
use std::{future::Future, task::Poll};
use tokio::sync::watch;

fn fixture() -> (
    tempfile::TempDir,
    watch::Sender<bool>,
    NativeUploads,
    UploadWriters,
) {
    let root = tempfile::tempdir().unwrap();
    let (stop, receiver) = watch::channel(false);
    let uploads = NativeUploads::new(
        Arc::new(UploadChunks::new(root.path().to_owned())),
        receiver,
    );
    (root, stop, uploads, UploadWriters::default())
}

#[tokio::test]
async fn another_scope_cannot_write_finish_or_abort_the_owner_chunk() {
    let (root, _stop, uploads, writers) = fixture();
    let owner = NativeRequestScope::default();
    let other = NativeRequestScope::default();
    let handle = writers
        .open(&uploads, &owner, "fixture", 0, 6)
        .await
        .unwrap();
    writers.write(&owner, &handle, b"abc").await.unwrap();
    assert!(writers.write(&other, &handle, b"bad").await.is_err());
    assert!(writers.finish(&other, &handle).await.is_err());
    writers.abort(&other, &handle);
    writers.write(&owner, &handle, b"def").await.unwrap();
    assert_eq!(writers.finish(&owner, &handle).await.unwrap(), 6);
    assert_eq!(
        std::fs::read(root.path().join("fixture/chunk_0")).unwrap(),
        b"abcdef"
    );
    assert_eq!(
        std::fs::read_dir(root.path().join("fixture"))
            .unwrap()
            .count(),
        1
    );
    assert!(writers.finish(&owner, &handle).await.is_err());
}

#[tokio::test]
async fn abort_removes_partial_bytes_and_rejects_later_writes_or_finish() {
    let (root, _stop, uploads, writers) = fixture();
    let owner = NativeRequestScope::default();
    let handle = writers
        .open(&uploads, &owner, "fixture", 0, 6)
        .await
        .unwrap();
    writers.write(&owner, &handle, b"abc").await.unwrap();
    writers.abort(&owner, &handle);
    assert!(writers.write(&owner, &handle, b"def").await.is_err());
    assert!(writers.finish(&owner, &handle).await.is_err());
    assert_eq!(
        std::fs::read_dir(root.path().join("fixture"))
            .unwrap()
            .count(),
        0
    );
}

#[tokio::test]
async fn closing_a_scope_aborts_only_its_chunks_and_refuses_new_opens() {
    let (root, _stop, uploads, writers) = fixture();
    let owner = NativeRequestScope::default();
    let other = NativeRequestScope::default();
    let first = writers
        .open(&uploads, &owner, "fixture", 0, 1)
        .await
        .unwrap();
    let second = writers
        .open(&uploads, &other, "fixture", 1, 1)
        .await
        .unwrap();
    writers.write(&owner, &first, b"a").await.unwrap();
    owner.close();
    writers.abort_owner(owner.id());
    assert!(writers.write(&owner, &first, b"a").await.is_err());
    assert!(writers.finish(&owner, &first).await.is_err());
    assert!(
        writers
            .open(&uploads, &owner, "fixture", 2, 1)
            .await
            .is_err()
    );
    writers.write(&other, &second, b"b").await.unwrap();
    assert_eq!(writers.finish(&other, &second).await.unwrap(), 1);
    assert!(!root.path().join("fixture/chunk_0").exists());
    assert_eq!(
        std::fs::read(root.path().join("fixture/chunk_1")).unwrap(),
        b"b"
    );
    assert_eq!(
        std::fs::read_dir(root.path().join("fixture"))
            .unwrap()
            .count(),
        1
    );
}

#[tokio::test]
async fn an_aborted_write_waiting_for_the_writer_lock_cannot_resume() {
    let (root, _stop, uploads, writers) = fixture();
    let owner = NativeRequestScope::default();
    let handle = writers
        .open(&uploads, &owner, "fixture", 0, 1)
        .await
        .unwrap();
    let slot = writers.owned(&owner, &handle).unwrap();
    let guard = slot.writer.lock().await;
    let mut write = Box::pin(writers.write(&owner, &handle, b"a"));
    assert!(std::future::poll_fn(|cx| Poll::Ready(write.as_mut().poll(cx).is_pending())).await);
    writers.abort(&owner, &handle);
    drop(guard);
    assert!(write.await.is_err());
    drop(slot);
    assert_eq!(
        std::fs::read_dir(root.path().join("fixture"))
            .unwrap()
            .count(),
        0
    );
}

#[tokio::test]
async fn incomplete_finish_drops_the_handle_and_never_publishes_a_chunk() {
    let (root, _stop, uploads, writers) = fixture();
    let owner = NativeRequestScope::default();
    let handle = writers
        .open(&uploads, &owner, "fixture", 0, 2)
        .await
        .unwrap();
    writers.write(&owner, &handle, b"a").await.unwrap();
    assert!(writers.finish(&owner, &handle).await.is_err());
    assert!(writers.write(&owner, &handle, b"b").await.is_err());
    assert_eq!(
        std::fs::read_dir(root.path().join("fixture"))
            .unwrap()
            .count(),
        0
    );
}

#[tokio::test]
async fn dropping_the_registry_removes_unfinished_owned_chunks() {
    let (root, _stop, uploads, writers) = fixture();
    let owner = NativeRequestScope::default();
    let handle = writers
        .open(&uploads, &owner, "fixture", 0, 2)
        .await
        .unwrap();
    writers.write(&owner, &handle, b"a").await.unwrap();
    drop(writers);
    assert_eq!(
        std::fs::read_dir(root.path().join("fixture"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn pending_open_cancellation_never_registers_or_leaves_late_temporary_files() {
    for cancellation in ["abort-owner", "close-scope", "drop-caller"] {
        let (root, _stop, uploads, writers) = fixture();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .unwrap();
        runtime.block_on(async {
            let owner = NativeRequestScope::default();
            let (release, blocked) = std::sync::mpsc::channel();
            let (started, ready) = tokio::sync::oneshot::channel();
            let blocker = tokio::task::spawn_blocking(move || {
                let _ = started.send(());
                let _ = blocked.recv();
            });
            ready.await.unwrap();
            let mut opening = Box::pin(writers.open(&uploads, &owner, "fixture", 0, 1));
            assert!(
                std::future::poll_fn(|cx| Poll::Ready(opening.as_mut().poll(cx).is_pending()))
                    .await
            );
            match cancellation {
                "abort-owner" => writers.abort_owner(owner.id()),
                "close-scope" => owner.close(),
                "drop-caller" => {}
                _ => unreachable!(),
            }
            if cancellation != "drop-caller" {
                let outcome =
                    std::future::poll_fn(|cx| Poll::Ready(opening.as_mut().poll(cx))).await;
                assert!(matches!(outcome, Poll::Ready(Err(_))), "{cancellation}");
            }
            drop(opening);
            release.send(()).unwrap();
            blocker.await.unwrap();
            let fresh = NativeRequestScope::default();
            let handle = writers
                .open(&uploads, &fresh, "fixture", 1, 1)
                .await
                .unwrap();
            writers.write(&fresh, &handle, b"b").await.unwrap();
            writers.finish(&fresh, &handle).await.unwrap();
            assert!(writers.registry.lock().unwrap().slots.is_empty());
        });
        drop(runtime);
        assert!(
            !root.path().join("fixture/chunk_0").exists(),
            "{cancellation}"
        );
        assert_eq!(
            std::fs::read(root.path().join("fixture/chunk_1")).unwrap(),
            b"b"
        );
        assert_eq!(
            std::fs::read_dir(root.path().join("fixture"))
                .unwrap()
                .count(),
            1,
            "{cancellation}"
        );
    }
}
