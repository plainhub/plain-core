use super::*;

#[tokio::test]
async fn failed_file_creation_preserves_existing_content() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("existing");
    std::fs::write(&path, b"keep existing content").unwrap();
    assert!(create_file(path.clone()).await.is_err());
    assert_eq!(std::fs::read(path).unwrap(), b"keep existing content");
}

#[tokio::test]
async fn created_file_is_removed_only_when_its_guard_is_dropped() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("owned");
    let (mut file, temporary) = create_file(path.clone()).await.unwrap();
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
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(".shared-fixture.part");
    runtime.block_on(async {
        let (started, ready) = tokio::sync::oneshot::channel();
        let (release, blocked) = std::sync::mpsc::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            started.send(()).unwrap();
            blocked.recv().unwrap();
        });
        ready.await.unwrap();
        let mut creation = Box::pin(create_file(path.clone()));
        assert!(futures_util::poll!(creation.as_mut()).is_pending());
        drop(creation);
        release.send(()).unwrap();
        blocker.await.unwrap();
        tokio::task::spawn_blocking(|| {}).await.unwrap();
    });
    runtime.shutdown_timeout(Duration::from_secs(5));
    assert!(!path.exists());
}

#[test]
fn progress_batches_chunks_but_always_reports_the_exact_final_size() {
    let mut progress = Progress::default();
    let reports = (1..=100)
        .filter(|bytes| progress.update(*bytes, 100, Duration::ZERO))
        .collect::<Vec<_>>();
    assert_eq!(reports, [100]);
    let mut progress = Progress::default();
    assert!(!progress.update(1, 10, Duration::from_millis(299)));
    assert!(progress.update(2, 10, Duration::from_millis(300)));
    assert!(!progress.update(3, 10, Duration::from_millis(599)));
    assert!(progress.update(4, 10, Duration::from_millis(600)));
    assert!(progress.update(10, 10, Duration::from_millis(601)));
}
#[test]
fn temporary_guard_removes_partial_file_and_generation_paths_cannot_escape_root() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("partial");
    std::fs::write(&path, b"partial").unwrap();
    drop(Temporary(path.clone()));
    assert!(!path.exists());
    assert!(directory(root.path(), "../outside").is_err());
    let generation = uuid::Uuid::new_v4().to_string();
    assert_eq!(
        directory(root.path(), &generation).unwrap(),
        root.path().join("shared-transfers").join(generation)
    );
}
#[tokio::test]
async fn cleanup_removes_only_the_owned_generation_and_is_repeatable() {
    let root = tempfile::tempdir().unwrap();
    let generation = uuid::Uuid::new_v4().to_string();
    let owned = directory(root.path(), &generation).unwrap();
    tokio::fs::create_dir_all(&owned).await.unwrap();
    tokio::fs::write(owned.join("file"), b"fixture")
        .await
        .unwrap();
    tokio::fs::write(root.path().join("keep"), b"keep")
        .await
        .unwrap();
    cleanup(root.path(), &generation).await.unwrap();
    cleanup(root.path(), &generation).await.unwrap();
    assert!(!owned.exists());
    assert!(root.path().join("keep").exists());
}
