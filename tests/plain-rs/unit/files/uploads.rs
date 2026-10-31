use super::*;
#[tokio::test]
async fn out_of_order_chunks_merge_by_index_and_success_removes_chunks() {
    let temp = tempfile::tempdir().unwrap();
    let chunks = temp.path().join("chunks");
    tokio::fs::create_dir(&chunks).await.unwrap();
    tokio::fs::write(chunks.join("chunk_1"), b"def")
        .await
        .unwrap();
    tokio::fs::write(chunks.join("chunk_0"), b"abc")
        .await
        .unwrap();
    let target = temp.path().join("out/file.txt");
    let result = merge(
        None,
        &chunks,
        2,
        6,
        Kind::File {
            path: target.clone(),
            replace: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(result, ("file.txt".into(), 6));
    assert_eq!(tokio::fs::read(target).await.unwrap(), b"abcdef");
    assert!(!chunks.exists());
}
#[tokio::test]
async fn missing_chunk_preserves_chunks_and_never_replaces_destination() {
    let temp = tempfile::tempdir().unwrap();
    let chunks = temp.path().join("chunks");
    tokio::fs::create_dir(&chunks).await.unwrap();
    tokio::fs::write(chunks.join("chunk_0"), b"abc")
        .await
        .unwrap();
    let target = temp.path().join("file");
    tokio::fs::write(&target, b"original").await.unwrap();
    assert!(
        merge(
            None,
            &chunks,
            2,
            6,
            Kind::File {
                path: target.clone(),
                replace: true
            }
        )
        .await
        .is_err()
    );
    assert!(chunks.exists());
    assert_eq!(tokio::fs::read(target).await.unwrap(), b"original");
}
#[tokio::test]
async fn size_mismatch_discards_stale_chunks_without_changing_target() {
    let temp = tempfile::tempdir().unwrap();
    let chunks = temp.path().join("chunks");
    tokio::fs::create_dir(&chunks).await.unwrap();
    tokio::fs::write(chunks.join("chunk_0"), b"abc")
        .await
        .unwrap();
    let target = temp.path().join("file");
    assert!(
        merge(
            None,
            &chunks,
            1,
            4,
            Kind::File {
                path: target.clone(),
                replace: true
            }
        )
        .await
        .is_err()
    );
    assert!(!chunks.exists());
    assert!(!target.exists());
}
#[test]
fn chunk_ids_cannot_escape_the_upload_directory() {
    for id in ["", ".", "..", "../a", "a/b", "a\\b", "a\0b"] {
        assert!(chunk_dir(Path::new("/tmp/uploads"), id).is_err());
    }
    assert_eq!(
        Runtime::default().status("unknown"),
        json!({"status":"NONE"})
    );
}

#[tokio::test]
async fn completion_failures_keep_the_committed_file_and_report_failure_once_without_retry() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let directory = tempfile::tempdir().unwrap();
    let db = Arc::new(crate::db::Db::open(&directory.path().join("plain.db")).unwrap());
    let store = FileStore::new(db, directory.path().into());
    let base = directory.path().join("chunks");
    save_chunk(&base, "upload", 0, b"complete").await.unwrap();
    let destination = directory.path().join("saved.txt");
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let runtime = Arc::new(Runtime::with_completion(move |path| {
        let count = count.clone();
        Box::pin(async move {
            count.fetch_add(1, Ordering::SeqCst);
            assert_eq!(tokio::fs::read(path).await.unwrap(), b"complete");
            Err(anyhow::anyhow!("scan refused"))
        })
    }));
    let (events, mut received) = broadcast::channel(8);
    runtime
        .start(
            store.clone(),
            base.clone(),
            "upload".into(),
            1,
            8,
            Kind::File {
                path: destination.clone(),
                replace: false,
            },
            events.clone(),
        )
        .await
        .unwrap();
    let event = tokio::time::timeout(std::time::Duration::from_secs(2), received.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(event.event_type, "UPLOAD_MERGE_RESULT");
    let payload: Value = serde_json::from_str(&event.payload).unwrap();
    assert_eq!(payload["ok"], false);
    let status = runtime.status("upload");
    assert_eq!(status["status"], "FAILED");
    assert!(
        status["error"]
            .as_str()
            .unwrap()
            .contains(destination.to_str().unwrap())
    );
    assert!(status["error"].as_str().unwrap().contains("scan refused"));
    assert!(!base.join("upload").exists());
    assert_eq!(tokio::fs::read(&destination).await.unwrap(), b"complete");
    assert!(
        runtime
            .start(
                store,
                base,
                "upload".into(),
                1,
                8,
                Kind::File {
                    path: destination,
                    replace: false
                },
                events
            )
            .await
            .is_err()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(received.try_recv().is_err());
}

#[tokio::test]
async fn saved_chunks_are_listed_in_numeric_order_with_sizes_and_reject_invalid_identity() {
    let directory = tempfile::tempdir().unwrap();
    assert!(list(directory.path(), "missing").await.unwrap().is_empty());
    for (index, bytes) in [
        (2, b"!".as_slice()),
        (0, b"hello".as_slice()),
        (1, b"world".as_slice()),
    ] {
        save_chunk(directory.path(), "upload", index, bytes)
            .await
            .unwrap();
    }
    assert_eq!(
        chunks(directory.path(), "upload").await.unwrap(),
        vec![(0, 5), (1, 5), (2, 1)]
    );
    assert_eq!(
        list(directory.path(), "upload").await.unwrap(),
        vec!["0:5", "1:5", "2:1"]
    );
    for id in ["", "../escape", "nested/child"] {
        assert!(save_chunk(directory.path(), id, 0, b"bytes").await.is_err());
    }
    assert!(
        save_chunk(directory.path(), "upload", -1, b"bytes")
            .await
            .is_err()
    );
}

#[tokio::test]
async fn replacement_and_unique_merge_commits_preserve_the_correct_existing_files() {
    for replace in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let chunks = directory.path().join("chunks");
        save_chunk(&chunks, "upload", 0, b"fresh").await.unwrap();
        let target = directory.path().join("report.txt");
        let variant = directory.path().join("report (1).txt");
        std::fs::write(&target, b"existing").unwrap();
        std::fs::write(&variant, b"variant").unwrap();
        let (saved, size) = merge(
            None,
            &chunks.join("upload"),
            1,
            5,
            Kind::File {
                path: target.clone(),
                replace,
            },
        )
        .await
        .unwrap();
        assert_eq!(size, 5);
        let saved = directory.path().join(saved);
        if replace {
            assert_eq!(saved, target);
            assert_eq!(std::fs::read(&target).unwrap(), b"fresh");
        } else {
            assert_eq!(saved, directory.path().join("report (2).txt"));
            assert_eq!(std::fs::read(&target).unwrap(), b"existing");
            assert_eq!(std::fs::read(saved).unwrap(), b"fresh");
        }
        assert_eq!(std::fs::read(&variant).unwrap(), b"variant");
        assert!(!chunks.join("upload").exists());
    }
}

#[test]
fn a_collision_after_merge_destination_selection_never_overwrites_the_other_writer() {
    let directory = tempfile::tempdir().unwrap();
    let original = directory.path().join("report.txt");
    let selected = crate::utils::unique_path::unique_sibling(&original).unwrap();
    let staged = directory.path().join("staged");
    std::fs::write(&staged, b"uploaded").unwrap();
    std::fs::write(&original, b"other writer").unwrap();
    let committed = commit_file(&staged, &original, selected, false).unwrap();
    assert_eq!(committed, directory.path().join("report (1).txt"));
    assert_eq!(std::fs::read(&original).unwrap(), b"other writer");
    assert_eq!(std::fs::read(&committed).unwrap(), b"uploaded");
    assert!(!staged.exists());
}

#[tokio::test]
async fn empty_chunk_atomically_replaces_existing_file_without_losing_raw_byte_support() {
    let directory = tempfile::tempdir().unwrap();
    let chunks = directory.path().join("chunks");
    tokio::fs::create_dir(&chunks).await.unwrap();
    tokio::fs::write(chunks.join("chunk_0"), []).await.unwrap();
    let target = directory.path().join("existing.bin");
    tokio::fs::write(&target, [0, 255, 128, 0]).await.unwrap();
    let (_, size) = merge(
        None,
        &chunks,
        1,
        0,
        Kind::File {
            path: target.clone(),
            replace: true,
        },
    )
    .await
    .unwrap();
    assert_eq!(size, 0);
    assert_eq!(tokio::fs::read(target).await.unwrap(), Vec::<u8>::new());
}

#[tokio::test]
async fn resource_commit_receives_complete_bytes_and_staging_is_removed_afterward() {
    let directory = tempfile::tempdir().unwrap();
    let chunks = directory.path().join("chunks");
    tokio::fs::create_dir(&chunks).await.unwrap();
    tokio::fs::write(chunks.join("chunk_0"), [0, 255])
        .await
        .unwrap();
    tokio::fs::write(chunks.join("chunk_1"), [128, 0])
        .await
        .unwrap();
    let staged = Arc::new(Mutex::new(None));
    let observed = staged.clone();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = calls.clone();
    let kind = Kind::Resource {
        name: "fixture.bin".into(),
        write: Arc::new(move |path| {
            let observed = observed.clone();
            let count = count.clone();
            Box::pin(async move {
                assert_eq!(tokio::fs::read(&path).await?, [0, 255, 128, 0]);
                *observed.lock().unwrap() = Some(path);
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            })
        }),
    };
    assert_eq!(
        merge(None, &chunks, 2, 4, kind).await.unwrap(),
        ("fixture.bin".into(), 4)
    );
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(!staged.lock().unwrap().as_ref().unwrap().exists());
}

#[tokio::test]
async fn incomplete_upload_never_opens_a_writable_resource() {
    let directory = tempfile::tempdir().unwrap();
    tokio::fs::write(directory.path().join("chunk_0"), [1])
        .await
        .unwrap();
    let kind = Kind::Resource {
        name: "fixture.bin".into(),
        write: Arc::new(|_| {
            Box::pin(async { panic!("Incomplete input must never reach the platform writer") })
        }),
    };
    assert!(merge(None, directory.path(), 2, 2, kind).await.is_err());
}

#[tokio::test]
async fn resource_failure_removes_merged_staging_and_preserves_input_chunks_for_retry() {
    let directory = tempfile::tempdir().unwrap();
    tokio::fs::write(directory.path().join("chunk_0"), [1, 255])
        .await
        .unwrap();
    let staged = Arc::new(Mutex::new(None));
    let observed = staged.clone();
    let kind = Kind::Resource {
        name: "fixture.bin".into(),
        write: Arc::new(move |path| {
            *observed.lock().unwrap() = Some(path);
            Box::pin(async { anyhow::bail!("Fixture resource write failure") })
        }),
    };
    let error = merge(None, directory.path(), 1, 2, kind).await.unwrap_err();
    assert!(error.to_string().contains("Fixture resource write failure"));
    assert!(!staged.lock().unwrap().as_ref().unwrap().exists());
    assert!(directory.path().join("chunk_0").exists());
}
