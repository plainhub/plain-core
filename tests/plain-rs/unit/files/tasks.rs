use super::*;
use std::sync::atomic::Ordering;
#[derive(Default)]
struct Recorder {
    events: Mutex<Vec<FileTask>>,
}
impl Events for Recorder {
    fn changed(&self, t: &FileTask) {
        self.events.lock().unwrap().push(t.clone());
    }
}
fn op(src: &Path, dst: &Path) -> FileTaskOp {
    FileTaskOp {
        src: src.to_str().unwrap().into(),
        dst: dst.to_str().unwrap().into(),
        overwrite: false,
    }
}
async fn terminal(service: &Service, cid: &str, id: &str) -> FileTask {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let task = service
                .list(cid)
                .unwrap()
                .into_iter()
                .find(|t| t.id == id)
                .unwrap();
            if matches!(task.status, FileTaskStatus::Done | FileTaskStatus::Error) {
                break task;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap()
}
#[tokio::test]
async fn owned_services_execute_multi_op_copy_and_directory_move_with_exact_cumulative_progress() {
    let events = Arc::new(Recorder::default());
    let service = Service::new(events.clone());
    let temp = tempfile::tempdir().unwrap();
    let a = temp.path().join("a");
    let b = temp.path().join("b");
    std::fs::write(&a, vec![7; 200_000]).unwrap();
    std::fs::write(&b, b"two").unwrap();
    let copy = service
        .create(
            "owner",
            FileTaskType::Copy,
            "copy",
            vec![
                op(&a, &temp.path().join("a-copy")),
                op(&b, &temp.path().join("b-copy")),
            ],
        )
        .unwrap();
    let done = terminal(&service, "owner", &copy.id).await;
    assert_eq!(done.status, FileTaskStatus::Done);
    assert_eq!(
        (
            done.total_bytes,
            done.done_bytes,
            done.total_items,
            done.done_items
        ),
        (200_003, 200_003, 2, 2)
    );
    assert_eq!(
        std::fs::read(temp.path().join("a-copy")).unwrap(),
        vec![7; 200_000]
    );
    assert_eq!(std::fs::read(temp.path().join("b-copy")).unwrap(), b"two");
    let directory = temp.path().join("dir");
    std::fs::create_dir_all(directory.join("empty")).unwrap();
    std::fs::write(directory.join("file"), b"directory").unwrap();
    let moved = service
        .create(
            "owner",
            FileTaskType::Move,
            "move",
            vec![op(&directory, &temp.path().join("moved"))],
        )
        .unwrap();
    let done = terminal(&service, "owner", &moved.id).await;
    assert_eq!(done.status, FileTaskStatus::Done);
    assert_eq!((done.done_bytes, done.done_items), (9, 1));
    assert!(!directory.exists());
    assert_eq!(
        std::fs::read(temp.path().join("moved/file")).unwrap(),
        b"directory"
    );
    assert!(temp.path().join("moved/empty").is_dir());
    assert!(service.list("other").unwrap().is_empty());
    let snapshots = events.events.lock().unwrap();
    let events = snapshots
        .iter()
        .filter(|t| t.id == copy.id)
        .collect::<Vec<_>>();
    assert_eq!(events.first().unwrap().status, FileTaskStatus::Queued);
    assert_eq!(events.last().unwrap().status, FileTaskStatus::Done);
    assert!(
        events
            .windows(2)
            .all(|pair| pair[1].done_bytes >= pair[0].done_bytes)
    );
}
#[tokio::test]
async fn bounded_admission_never_records_a_rejected_task() {
    let events = Arc::new(Recorder::default());
    let service = Service::new(events.clone());
    let temp = tempfile::tempdir().unwrap();
    let mut admitted = 0;
    loop {
        match service.create(
            "owner",
            FileTaskType::Copy,
            "copy",
            vec![op(&temp.path().join("absent"), &temp.path().join("out"))],
        ) {
            Ok(_) => {
                admitted += 1;
                assert!(admitted < 128);
            }
            Err(error) => {
                assert!(error.to_string().contains("queue"));
                break;
            }
        }
    }
    assert_eq!(service.list("owner").unwrap().len(), admitted);
    assert_eq!(events.events.lock().unwrap().len(), admitted);
}
#[test]
fn a_closed_worker_queue_rejects_new_work_without_queued_records() {
    let events = Arc::new(Recorder::default());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let service = runtime.block_on(async { Service::new(events.clone()) });
    drop(runtime);
    assert!(
        service
            .create(
                "owner",
                FileTaskType::Copy,
                "copy",
                vec![FileTaskOp {
                    src: "/synthetic/source".into(),
                    dst: "/synthetic/destination".into(),
                    overwrite: false
                }]
            )
            .is_err()
    );
    assert!(service.list("owner").unwrap().is_empty());
    assert!(events.events.lock().unwrap().is_empty());
}
struct ScanFailure;
impl Hooks for ScanFailure {
    fn authorize<'a>(&'a self, _: FileTaskType, _: &'a [FileTaskOp]) -> HookResult<'a> {
        Box::pin(async { Ok(()) })
    }
    fn completed<'a>(&'a self, _: FileTaskType, _: &'a CompletedOp) -> HookResult<'a> {
        Box::pin(async { bail!("synthetic scan failure") })
    }
}
#[tokio::test]
async fn completed_operations_keep_resolved_collision_paths_when_native_scan_fails() {
    let service = Service::with_hooks(Arc::new(Recorder::default()), Arc::new(ScanFailure));
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let target = temp.path().join("copy.txt");
    std::fs::write(&source, b"synthetic").unwrap();
    std::fs::write(&target, b"existing").unwrap();
    let task = service
        .create(
            "owner",
            FileTaskType::Copy,
            "copy",
            vec![op(&source, &target)],
        )
        .unwrap();
    let task = terminal(&service, "owner", &task.id).await;
    assert_eq!(task.status, FileTaskStatus::Error);
    assert!(task.error.contains("scan failure"));
    assert_eq!(task.completed_ops.len(), 1);
    assert_eq!(
        task.completed_ops[0].dst,
        temp.path().join("copy (1).txt").to_str().unwrap()
    );
    assert_eq!(std::fs::read(&target).unwrap(), b"existing");
    assert_eq!(
        std::fs::read(&task.completed_ops[0].dst).unwrap(),
        b"synthetic"
    );
    assert!(service.list("foreign").unwrap().is_empty());
    let retained = service.list("owner").unwrap();
    assert_eq!(retained.len(), 1);
    assert_eq!(retained[0].id, task.id);
    assert_eq!(retained[0].completed_ops.len(), task.completed_ops.len());
    assert_eq!(retained[0].completed_ops[0].dst, task.completed_ops[0].dst);
}
struct RevokedBeforeOperation(std::sync::atomic::AtomicUsize);
impl Hooks for RevokedBeforeOperation {
    fn authorize<'a>(&'a self, _: FileTaskType, _: &'a [FileTaskOp]) -> HookResult<'a> {
        Box::pin(async move {
            if self.0.fetch_add(1, Ordering::SeqCst) > 0 {
                bail!("synthetic permission revoked");
            }
            Ok(())
        })
    }
    fn completed<'a>(&'a self, _: FileTaskType, _: &'a CompletedOp) -> HookResult<'a> {
        Box::pin(async { Ok(()) })
    }
}
#[tokio::test]
async fn permission_revoked_after_measurement_prevents_physical_execution() {
    let service = Service::with_hooks(
        Arc::new(Recorder::default()),
        Arc::new(RevokedBeforeOperation(std::sync::atomic::AtomicUsize::new(
            0,
        ))),
    );
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let target = temp.path().join("target");
    std::fs::write(&source, b"synthetic").unwrap();
    let task = service
        .create(
            "owner",
            FileTaskType::Move,
            "move",
            vec![op(&source, &target)],
        )
        .unwrap();
    let failed = terminal(&service, "owner", &task.id).await;
    assert_eq!(failed.status, FileTaskStatus::Error);
    assert!(failed.error.contains("permission revoked"));
    assert!(failed.completed_ops.is_empty());
    assert_eq!(std::fs::read(&source).unwrap(), b"synthetic");
    assert!(!target.exists());
}
#[cfg(unix)]
#[tokio::test]
async fn special_files_fail_without_blocking_the_worker() {
    let service = Service::new(Arc::new(Recorder::default()));
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("socket");
    let _socket = std::os::unix::net::UnixListener::bind(&source).unwrap();
    let target = temp.path().join("target");
    let task = service
        .create(
            "owner",
            FileTaskType::Copy,
            "copy",
            vec![op(&source, &target)],
        )
        .unwrap();
    let failed = terminal(&service, "owner", &task.id).await;
    assert_eq!(failed.status, FileTaskStatus::Error);
    assert!(failed.error.contains("unsupported file type"));
    assert!(!target.exists());
}

#[tokio::test]
async fn tasks_are_lost_when_the_service_is_recreated_and_failures_are_not_retried() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let target = temp.path().join("target");
    std::fs::write(&source, b"synthetic").unwrap();
    let events = Arc::new(Recorder::default());
    let service = Service::with_hooks(events.clone(), Arc::new(ScanFailure));
    let queued = service
        .create(
            "owner",
            FileTaskType::Move,
            "move",
            vec![op(&source, &target)],
        )
        .unwrap();
    let failed = terminal(&service, "owner", &queued.id).await;
    assert_eq!(failed.status, FileTaskStatus::Error);
    assert_eq!(failed.completed_ops.len(), 1);
    assert!(!source.exists());
    assert_eq!(std::fs::read(&target).unwrap(), b"synthetic");
    assert_eq!(
        events
            .events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| event.status == FileTaskStatus::Error)
            .count(),
        1
    );
    assert_eq!(
        service.get(&queued.id).unwrap().unwrap().error,
        failed.error
    );
    assert!(service.list("foreign").unwrap().is_empty());
    drop(service);
    let restarted = Service::new(Arc::new(Recorder::default()));
    assert!(restarted.list("owner").unwrap().is_empty());
    assert!(restarted.get(&queued.id).unwrap().is_none());
}
#[tokio::test]
async fn invalid_sources_fail_and_later_tasks_still_run() {
    let service = Service::new(Arc::new(Recorder::default()));
    let temp = tempfile::tempdir().unwrap();
    let absent = temp.path().join("absent");
    let target = temp.path().join("target");
    let task = service
        .create(
            "owner",
            FileTaskType::Copy,
            "copy",
            vec![op(&absent, &target)],
        )
        .unwrap();
    assert_eq!(
        terminal(&service, "owner", &task.id).await.status,
        FileTaskStatus::Error
    );
    assert!(!target.exists());
    std::fs::write(&absent, b"now exists").unwrap();
    assert_eq!(
        service.get(&task.id).unwrap().unwrap().status,
        FileTaskStatus::Error
    );
    let next = service
        .create(
            "owner",
            FileTaskType::Copy,
            "copy",
            vec![op(&absent, &target)],
        )
        .unwrap();
    assert_eq!(
        terminal(&service, "owner", &next.id).await.status,
        FileTaskStatus::Done
    );
}
#[tokio::test]
async fn rename_uses_the_exact_target_and_does_not_choose_a_collision_name() {
    let service = Service::new(Arc::new(Recorder::default()));
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let target = temp.path().join("target");
    std::fs::write(&source, b"source").unwrap();
    std::fs::write(&target, b"existing").unwrap();
    let failed = service
        .rename(
            "owner",
            source.to_str().unwrap().into(),
            target.to_str().unwrap().into(),
        )
        .unwrap();
    assert_eq!(
        terminal(&service, "owner", &failed.id).await.status,
        FileTaskStatus::Error
    );
    assert_eq!(std::fs::read(&source).unwrap(), b"source");
    assert_eq!(std::fs::read(&target).unwrap(), b"existing");
    let new = temp.path().join("new");
    let renamed = service
        .rename(
            "owner",
            source.to_str().unwrap().into(),
            new.to_str().unwrap().into(),
        )
        .unwrap();
    let done = terminal(&service, "owner", &renamed.id).await;
    assert_eq!(done.status, FileTaskStatus::Done);
    assert_eq!((done.done_bytes, done.done_items), (6, 1));
    assert_eq!(done.completed_ops[0].dst, new.to_str().unwrap());
    assert!(!source.exists());
}
struct FailSecondCompletion(std::sync::atomic::AtomicUsize);
impl Hooks for FailSecondCompletion {
    fn authorize<'a>(&'a self, _: FileTaskType, _: &'a [FileTaskOp]) -> HookResult<'a> {
        Box::pin(async { Ok(()) })
    }
    fn completed<'a>(&'a self, _: FileTaskType, _: &'a CompletedOp) -> HookResult<'a> {
        Box::pin(async move {
            if self.0.fetch_add(1, Ordering::SeqCst) == 1 {
                bail!("synthetic second completion failure");
            }
            Ok(())
        })
    }
}
#[tokio::test]
async fn failed_postprocessing_preserves_partial_results_and_stops_remaining_operations() {
    let temp = tempfile::tempdir().unwrap();
    let hooks = Arc::new(FailSecondCompletion(std::sync::atomic::AtomicUsize::new(0)));
    let service = Service::with_hooks(Arc::new(Recorder::default()), hooks.clone());
    let mut operations = Vec::new();
    for name in ["first", "second", "third"] {
        let source = temp.path().join(name);
        std::fs::write(&source, b"fixture").unwrap();
        operations.push(op(&source, &temp.path().join(format!("{name}-moved"))));
    }
    let task = service
        .create("owner", FileTaskType::Move, "move", operations)
        .unwrap();
    let failed = terminal(&service, "owner", &task.id).await;
    assert_eq!(failed.status, FileTaskStatus::Error);
    assert!(failed.error.contains("second completion failure"));
    assert_eq!(failed.completed_ops.len(), 2);
    assert_eq!(
        (
            failed.done_bytes,
            failed.total_bytes,
            failed.done_items,
            failed.total_items
        ),
        (14, 21, 2, 3)
    );
    assert_eq!(hooks.0.load(Ordering::SeqCst), 2);
    assert!(!temp.path().join("first").exists());
    assert!(!temp.path().join("second").exists());
    assert_eq!(
        std::fs::read(temp.path().join("third")).unwrap(),
        b"fixture"
    );
    assert!(!temp.path().join("third-moved").exists());
}
