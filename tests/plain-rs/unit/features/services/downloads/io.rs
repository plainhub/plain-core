use super::*;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

#[tokio::test]
async fn failed_file_creation_preserves_existing_content() {
    let work = BlockingWork::default();
    let root = plain_libs::temporary::tempdir().unwrap();
    let path = root.path().join("existing");
    std::fs::write(&path, b"keep existing content").unwrap();
    assert!(create_file(&work, path.clone()).await.is_err());
    assert_eq!(std::fs::read(path).unwrap(), b"keep existing content");
}

#[tokio::test]
async fn created_file_is_removed_only_when_its_guard_is_dropped() {
    let work = BlockingWork::default();
    let root = plain_libs::temporary::tempdir().unwrap();
    let path = root.path().join("owned");
    let (mut file, temporary) = create_file(&work, path.clone()).await.unwrap();
    file.write_all(b"owned content").await.unwrap();
    file.flush().await.unwrap();
    drop(file);
    assert_eq!(std::fs::read(&path).unwrap(), b"owned content");
    drop(temporary);
    assert!(!path.exists());
}

#[test]
fn canceled_queued_file_creation_cannot_leave_a_partial_file() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    let work = BlockingWork::default();
    let root = plain_libs::temporary::tempdir().unwrap();
    let path = root.path().join(".shared-fixture.part");
    runtime.block_on(async {
        let (started, ready) = tokio::sync::oneshot::channel();
        let (release, blocked) = std::sync::mpsc::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            started.send(()).unwrap();
            blocked.recv().unwrap();
        });
        ready.await.unwrap();
        let mut creation = Box::pin(create_file(&work, path.clone()));
        assert!(futures_util::poll!(creation.as_mut()).is_pending());
        drop(creation);
        let mut finish = Box::pin(work.finish());
        assert!(futures_util::poll!(finish.as_mut()).is_pending());
        drop(finish);
        release.send(()).unwrap();
        blocker.await.unwrap();
        work.finish().await.unwrap();
        assert!(!path.exists());
    });
    runtime.shutdown_timeout(Duration::from_secs(5));
    assert!(!path.exists());
}
