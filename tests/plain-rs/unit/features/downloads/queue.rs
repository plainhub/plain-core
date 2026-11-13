use super::*;

fn create(queue: &Queue, name: &str) -> Download {
    queue
        .create(
            Source::Resources {
                urls: vec![format!(
                    "https://fixture.invalid/fs?id=s.owner.{name}&action=download"
                )],
            },
            "/downloads".into(),
            None,
            name.into(),
            vec![],
            vec![],
        )
        .unwrap()
}

#[test]
fn pending_work_is_fifo_and_repeated_active_requests_converge() {
    let queue = Queue::default();
    let first = create(&queue, "first");
    let second = create(&queue, "second");
    assert_eq!(create(&queue, "first").id, first.id);
    assert_eq!(queue.claim().unwrap().download.id, first.id);
    assert_eq!(queue.claim().unwrap().download.id, second.id);
    assert!(queue.claim().is_none());
}

#[test]
fn pause_invalidates_bytes_and_resume_preserves_only_committed_files() {
    let queue = Queue::default();
    let download = create(&queue, "file");
    let first = queue.claim().unwrap();
    queue
        .plan(&download.id, &first.generation, 2, 12, "two files")
        .unwrap();
    queue
        .begin_file(&download.id, &first.generation, "first", 5)
        .unwrap();
    queue.progress(&download.id, &first.generation, 5).unwrap();
    queue
        .finish_file(&download.id, &first.generation, "first", || Ok(()))
        .unwrap();
    queue
        .begin_file(&download.id, &first.generation, "second", 7)
        .unwrap();
    queue.progress(&download.id, &first.generation, 3).unwrap();
    assert!(queue.control(&download.id, Command::Pause).unwrap());
    assert!(queue.progress(&download.id, &first.generation, 7).is_err());
    assert!(
        !queue
            .finish(&download.id, &first.generation, Ok(()))
            .unwrap()
    );
    let snapshot = &queue.snapshot()[0];
    assert_eq!(snapshot.status, Status::Paused);
    assert_eq!(snapshot.downloaded_bytes, 5);
    assert_eq!(snapshot.done_files, 1);
    assert!(queue.control(&download.id, Command::Resume).unwrap());
    let next = queue.claim().unwrap();
    assert_ne!(next.generation, first.generation);
    assert_eq!(next.completed["first"], 5);
    assert!(!next.completed.contains_key("second"));
}

#[test]
fn invalid_or_incomplete_progress_does_not_become_success() {
    let queue = Queue::default();
    let download = create(&queue, "file");
    let run = queue.claim().unwrap();
    queue
        .plan(&download.id, &run.generation, 1, 7, "file")
        .unwrap();
    queue
        .begin_file(&download.id, &run.generation, "file", 7)
        .unwrap();
    assert!(queue.progress(&download.id, &run.generation, 8).is_err());
    queue.progress(&download.id, &run.generation, 3).unwrap();
    assert!(queue.progress(&download.id, &run.generation, 2).is_err());
    assert!(
        queue
            .finish_file(&download.id, &run.generation, "file", || Ok(()))
            .is_err()
    );
    assert_eq!(queue.snapshot()[0].done_files, 0);
}

#[test]
fn partial_failure_retries_without_repeating_successful_file_effects() {
    let queue = Queue::default();
    let download = create(&queue, "file");
    let run = queue.claim().unwrap();
    queue
        .plan(&download.id, &run.generation, 2, 7, "files")
        .unwrap();
    queue
        .begin_file(&download.id, &run.generation, "a", 3)
        .unwrap();
    queue.progress(&download.id, &run.generation, 3).unwrap();
    queue
        .finish_file(&download.id, &run.generation, "a", || Ok(()))
        .unwrap();
    queue
        .begin_file(&download.id, &run.generation, "b", 4)
        .unwrap();
    queue
        .fail_file(
            &download.id,
            &run.generation,
            "b",
            plain_libs::anyhow::anyhow!("offline"),
        )
        .unwrap();
    queue.finish(&download.id, &run.generation, Ok(())).unwrap();
    assert_eq!(queue.snapshot()[0].status, Status::Partial);
    assert_eq!(queue.snapshot()[0].failures[0].path, "b");
    assert!(queue.control(&download.id, Command::Retry).unwrap());
    let retry = queue.claim().unwrap();
    assert_eq!(retry.completed.len(), 1);
    assert_eq!(retry.completed["a"], 3);
    assert!(retry.download.failures.is_empty());
}

#[test]
fn controls_never_create_missing_tasks_and_cancel_keeps_a_removable_record() {
    let queue = Queue::default();
    for command in [
        Command::Pause,
        Command::Resume,
        Command::Retry,
        Command::Cancel,
        Command::Remove,
    ] {
        assert!(!queue.control("missing", command).unwrap());
    }
    let download = create(&queue, "file");
    assert!(queue.control(&download.id, Command::Cancel).unwrap());
    assert_eq!(queue.snapshot()[0].status, Status::Canceled);
    assert!(queue.claim().is_none());
    assert!(queue.control(&download.id, Command::Remove).unwrap());
    assert!(queue.snapshot().is_empty());
}

#[test]
fn incomplete_transfers_do_not_execute_import_commit_effects() {
    let queue = Queue::default();
    let download = create(&queue, "file");
    let run = queue.claim().unwrap();
    queue
        .begin_file(&download.id, &run.generation, "file", 3)
        .unwrap();
    queue.progress(&download.id, &run.generation, 2).unwrap();
    let mut commits = 0;
    assert!(
        queue
            .finish_file(&download.id, &run.generation, "file", || {
                commits += 1;
                Ok(())
            })
            .is_err()
    );
    assert_eq!(commits, 0);
}

#[test]
fn completed_history_does_not_consume_pending_capacity() {
    let queue = Queue::default();
    for index in 0..150 {
        let download = create(&queue, &format!("file{index}"));
        let run = queue.claim().unwrap();
        queue
            .begin_file(&download.id, &run.generation, "file", 0)
            .unwrap();
        queue
            .finish_file(&download.id, &run.generation, "file", || Ok(()))
            .unwrap();
        queue.finish(&download.id, &run.generation, Ok(())).unwrap();
    }
    assert_eq!(queue.snapshot().len(), 150);
    assert!(
        queue
            .snapshot()
            .iter()
            .all(|download| download.status == Status::Completed)
    );
}

#[test]
fn explicit_retry_is_consumed_by_one_accepted_generation() {
    use crate::infrastructure::transport::router::Attempt;
    let queue = Queue::default();
    let download = create(&queue, "file");
    let run = queue.claim().unwrap();
    assert_eq!(run.attempt, Attempt::Automatic);
    queue
        .finish(
            &download.id,
            &run.generation,
            Err(plain_libs::anyhow::anyhow!("offline")),
        )
        .unwrap();
    queue.control(&download.id, Command::Retry).unwrap();
    let retry = queue.claim().unwrap();
    assert_eq!(retry.attempt, Attempt::Retry);
    queue.control(&download.id, Command::Pause).unwrap();
    queue.control(&download.id, Command::Resume).unwrap();
    assert_eq!(queue.claim().unwrap().attempt, Attempt::Automatic);
}

#[test]
fn canceled_files_and_archives_never_execute_commit_effects() {
    let queue = Queue::default();
    let download = create(&queue, "canceled");
    let run = queue.claim().unwrap();
    queue
        .begin_file(&download.id, &run.generation, "file", 0)
        .unwrap();
    queue.control(&download.id, Command::Cancel).unwrap();
    let mut effects = 0;
    assert!(
        queue
            .finish_file(&download.id, &run.generation, "file", || {
                effects += 1;
                Ok(())
            })
            .is_err()
    );
    assert!(
        queue
            .commit(&download.id, &run.generation, || {
                effects += 1;
                Ok(())
            })
            .is_err()
    );
    assert_eq!(effects, 0);
}

#[test]
fn progress_notifications_are_batched_but_exact_final_bytes_are_always_reported() {
    let queue = Queue::default();
    let download = create(&queue, "progress");
    let run = queue.claim().unwrap();
    queue
        .begin_file(&download.id, &run.generation, "file", 100)
        .unwrap();
    let mut state = queue.state.lock().unwrap();
    let task = state.tasks.get_mut(&download.id).unwrap();
    let reports: Vec<_> = (1..=100)
        .filter(|bytes| progress::advance(task, *bytes, std::time::Duration::ZERO).unwrap())
        .collect();
    assert_eq!(reports, [100]);
    assert_eq!(task.run.download.downloaded_bytes, 100);
    task.current_size = 110;
    assert!(!progress::advance(task, 101, std::time::Duration::from_millis(299)).unwrap());
    assert!(progress::advance(task, 102, std::time::Duration::from_millis(300)).unwrap());
    assert!(!progress::advance(task, 103, std::time::Duration::from_millis(299)).unwrap());
    assert!(progress::advance(task, 110, std::time::Duration::from_millis(1)).unwrap());
}

#[test]
fn archive_failure_rolls_back_staged_file_counts_and_retry_rebuilds_the_archive() {
    let queue = Queue::default();
    let download = queue
        .create(
            Source::Resources {
                urls: vec!["fixture".into()],
            },
            "/downloads".into(),
            Some("files.zip".into()),
            "files".into(),
            vec![],
            vec![],
        )
        .unwrap();
    let run = queue.claim().unwrap();
    queue
        .begin_file(&download.id, &run.generation, "file", 0)
        .unwrap();
    queue
        .finish_file(&download.id, &run.generation, "file", || Ok(()))
        .unwrap();
    queue.packing(&download.id, &run.generation).unwrap();
    queue
        .finish(
            &download.id,
            &run.generation,
            Err(plain_libs::anyhow::anyhow!("archive refused")),
        )
        .unwrap();
    let task = &queue.snapshot()[0];
    assert_eq!(task.status, Status::Failed);
    assert_eq!((task.done_files, task.downloaded_bytes), (0, 0));
    queue.control(&download.id, Command::Retry).unwrap();
    assert!(queue.claim().unwrap().completed.is_empty());
}

#[test]
fn retry_cannot_bypass_the_active_task_capacity() {
    let queue = Queue::default();
    let mut canceled = vec![];
    for number in 0..129 {
        let task = create(&queue, &number.to_string());
        queue.control(&task.id, Command::Cancel).unwrap();
        canceled.push(task.id);
    }
    for id in &canceled[..128] {
        assert!(queue.control(id, Command::Retry).unwrap());
    }
    assert!(queue.control(&canceled[128], Command::Retry).is_err());
    assert_eq!(
        queue
            .snapshot()
            .iter()
            .find(|task| task.id == canceled[128])
            .unwrap()
            .status,
        Status::Canceled
    );
    queue.control(&canceled[0], Command::Cancel).unwrap();
    assert!(queue.control(&canceled[128], Command::Retry).unwrap());
}
