use super::*;

#[tokio::test]
async fn volumes_queue_in_order_deduplicate_and_keep_controls_scoped() {
    let scans = Arc::new(MediaScans::default());
    let first = scans.enqueue_volume("first").unwrap().unwrap();
    let second = scans.enqueue_volume("second").unwrap().unwrap();
    let third = scans.enqueue_volume("third").unwrap().unwrap();
    assert!(scans.enqueue_volume("second").unwrap().is_none());
    assert_eq!(
        scans.snapshot_for("first").unwrap().unwrap().status,
        Status::Running
    );
    assert_eq!(
        scans.snapshot_for("second").unwrap().unwrap().status,
        Status::Queued
    );
    assert!(first.record(|| Ok(())).is_ok());
    assert!(second.record(|| Ok(())).is_err());
    scans.pause(Some("second")).unwrap();
    first.finish(Ok(())).unwrap();
    assert_eq!(
        scans.snapshot_for("second").unwrap().unwrap().status,
        Status::Paused
    );
    assert_eq!(
        scans.snapshot_for("third").unwrap().unwrap().status,
        Status::Running
    );
    scans.resume(Some("second")).unwrap();
    assert_eq!(
        scans.snapshot_for("second").unwrap().unwrap().status,
        Status::Queued
    );
    scans.cancel(Some("third")).unwrap();
    assert_eq!(third.finish(Ok(())).unwrap().status, Status::Canceled);
    assert_eq!(
        scans.snapshot_for("second").unwrap().unwrap().status,
        Status::Running
    );
    second.record_when_running(2, || Ok(())).await.unwrap();
    second.finish(Ok(())).unwrap();
    assert_eq!(
        scans
            .snapshot_for("first")
            .unwrap()
            .unwrap()
            .processed_files,
        1
    );
    assert_eq!(
        scans
            .snapshot_for("second")
            .unwrap()
            .unwrap()
            .processed_files,
        2
    );
    assert!(scans.enqueue_volume("first").unwrap().is_some());
}

#[tokio::test]
async fn pause_running_yields_the_slot_and_global_controls_cover_the_queue() {
    let scans = Arc::new(MediaScans::default());
    let first = scans.enqueue_volume("first").unwrap().unwrap();
    let second = scans.enqueue_volume("second").unwrap().unwrap();
    scans.pause(Some("first")).unwrap();
    assert_eq!(
        scans.snapshot_for("second").unwrap().unwrap().status,
        Status::Running
    );
    assert!(first.record(|| Ok(())).is_err());
    scans.pause(None).unwrap();
    assert_eq!(
        scans.snapshot_for("second").unwrap().unwrap().status,
        Status::Paused
    );
    scans.resume(None).unwrap();
    assert_eq!(
        scans.snapshot_for("first").unwrap().unwrap().status,
        Status::Running
    );
    assert_eq!(
        scans.snapshot_for("second").unwrap().unwrap().status,
        Status::Queued
    );
    scans.cancel(None).unwrap();
    assert_eq!(
        scans.snapshot_for("second").unwrap().unwrap().status,
        Status::Canceled
    );
    first.finish(Ok(())).unwrap();
    assert!(scans.wait_idle().await.unwrap());
    assert!(second.wait_running().await.is_err());
    scans.close().unwrap();
    assert!(scans.enqueue_volume("third").is_err());
}

#[test]
fn failure_and_drop_release_the_next_volume_without_erasing_results() {
    let scans = Arc::new(MediaScans::default());
    let first = scans.enqueue_volume("first").unwrap().unwrap();
    let second = scans.enqueue_volume("second").unwrap().unwrap();
    let third = scans.enqueue_volume("third").unwrap().unwrap();
    first.finish(Err("generated failure".into())).unwrap();
    assert_eq!(
        scans.snapshot_for("second").unwrap().unwrap().status,
        Status::Running
    );
    drop(second);
    assert_eq!(
        scans.snapshot_for("third").unwrap().unwrap().status,
        Status::Running
    );
    third.finish(Ok(())).unwrap();
    assert_eq!(
        scans
            .snapshot_for("first")
            .unwrap()
            .unwrap()
            .error
            .as_deref(),
        Some("generated failure")
    );
}

#[test]
fn volume_events_report_the_changed_volume_and_next_task_without_repeating_completed_results() {
    let (events, mut received) = tokio::sync::broadcast::channel(32);
    let scans = Arc::new(MediaScans::new(events, std::env::temp_dir()));
    let first = scans.enqueue_volume("first").unwrap().unwrap();
    let second = scans.enqueue_volume("second").unwrap().unwrap();
    let mut queued = false;
    while let Ok(event) = received.try_recv() {
        let payload: plain_libs::serde_json::Value =
            plain_libs::serde_json::from_str(&event.payload).unwrap();
        queued |= payload["volumeId"] == "second" && payload["state"] == "QUEUED";
    }
    assert!(queued);
    first.finish(Ok(())).unwrap();
    let mut transitions = Vec::new();
    while let Ok(event) = received.try_recv() {
        let payload: plain_libs::serde_json::Value =
            plain_libs::serde_json::from_str(&event.payload).unwrap();
        transitions.push((
            payload["volumeId"].as_str().unwrap().to_owned(),
            payload["state"].as_str().unwrap().to_owned(),
        ));
    }
    assert_eq!(
        transitions,
        [
            ("first".into(), "IDLE".into()),
            ("second".into(), "RUNNING".into())
        ]
    );
    second.record(|| Ok(())).unwrap();
    let payload: plain_libs::serde_json::Value =
        plain_libs::serde_json::from_str(&received.try_recv().unwrap().payload).unwrap();
    assert_eq!(payload["volumeId"], "second");
    assert_eq!(payload["indexed"], 1);
    assert!(received.try_recv().is_err());
}
