use super::*;
fn fixture(count: usize) -> (tempfile::TempDir, Db, Queue) {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("plain.db")).unwrap();
    let peer = DPeer::new(
        "peer",
        "fixture",
        "127.0.0.1",
        Some(443),
        crate::chat::enums::DeviceType::Phone,
    );
    peers::save(&db, &[peer], crate::db::chat_store::SaveMode::Insert).unwrap();
    let items=(0..count).map(|n|json!({"id":n.to_string(),"uri":format!("fsid:remote-{n}"),"fileName":"fixture.txt","size":3})).collect::<Vec<_>>();
    let mut chat = crate::db::DChat::new(
        "me",
        "peer",
        "",
        &json!({"type":"FILES","value":{"items":items}}).to_string(),
    );
    chat.id = "message".into();
    db.insert_chat(&chat);
    let queue = Queue::new(
        db.clone(),
        dir.path().to_path_buf(),
        Arc::new(Imports::default()),
    );
    (dir, db, queue)
}
fn status(queue: &Queue, id: &str) -> String {
    queue.snapshot()["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"] == id)
        .unwrap()["status"]
        .as_str()
        .unwrap()
        .into()
}
fn start(queue: &Queue) -> (Snapshot, Ticket) {
    match queue.effect().unwrap() {
        Effect::Start { task, ticket, .. } => (task, ticket),
        _ => panic!("expected start"),
    }
}
#[test]
fn fifo_three_slots_pause_cancel_and_stale_receipts() {
    let (_dir, _db, q) = fixture(5);
    for n in 0..5 {
        assert!(q.enqueue("message", &n.to_string(), "peer").unwrap());
    }
    let (old, _) = start(&q);
    assert_eq!(old.id, "0");
    assert_eq!(start(&q).0.id, "1");
    assert_eq!(start(&q).0.id, "2");
    assert!(q.effect().is_none());
    assert_eq!(status(&q, "3"), "PENDING");
    assert!(!q.enqueue("message", "0", "peer").unwrap());
    assert!(q.control("0", "pause").unwrap());
    assert!(matches!(q.effect().unwrap(),Effect::Cancel{token} if token==old.generation));
    let (fourth, ticket) = start(&q);
    assert_eq!(fourth.id, "3");
    assert!(!q.progress(&old.id, &old.generation, 3).unwrap());
    assert!(!q.finish(&old.id, &old.generation, None).unwrap());
    assert_eq!(status(&q, "0"), "PAUSED");
    std::fs::write(&ticket.path, b"abc").unwrap();
    assert!(q.finish(&fourth.id, &fourth.generation, None).unwrap());
    assert_eq!(start(&q).0.id, "4");
    assert!(q.control("0", "resume").unwrap());
    assert_eq!(status(&q, "0"), "PENDING");
    assert!(q.control("1", "cancel").unwrap());
    assert!(matches!(q.effect().unwrap(), Effect::Cancel { .. }));
    let (new, _) = start(&q);
    assert_eq!(new.id, "0");
    assert_ne!(old.generation, new.generation);
    assert!(
        !q.finish(&old.id, &old.generation, Some("late".into()))
            .unwrap()
    );
    assert_eq!(status(&q, "0"), "DOWNLOADING");
}
#[test]
fn bytes_are_monotonic_and_sql_failure_never_completes() {
    let (_dir, db, q) = fixture(1);
    q.enqueue("message", "0", "peer").unwrap();
    let (task, ticket) = start(&q);
    assert!(q.progress("0", &task.generation, 2).unwrap());
    assert!(q.progress("0", &task.generation, 1).is_err());
    assert!(q.progress("0", &task.generation, 4).is_err());
    std::fs::write(&ticket.path, b"abc").unwrap();
    db.with_conn(|c|c.execute_batch("CREATE TRIGGER reject BEFORE INSERT ON app_files BEGIN SELECT RAISE(ABORT,'fixture'); END;")).unwrap();
    q.finish("0", &task.generation, None).unwrap();
    assert_eq!(status(&q, "0"), "FAILED");
    assert!(!std::path::Path::new(&ticket.path).exists());
    assert_eq!(
        db.with_conn(|c| c.query_row("SELECT count(*) FROM app_files", [], |r| r.get::<_, i64>(0)))
            .unwrap(),
        0
    );
    assert!(q.control("0", "retry").unwrap());
    let (new, ticket) = start(&q);
    assert_ne!(new.generation, task.generation);
    assert!(!q.progress("0", &task.generation, 3).unwrap());
    assert_eq!(q.snapshot()["tasks"][0]["downloaded"], 0);
    db.with_conn(|c| c.execute_batch("DROP TRIGGER reject"))
        .unwrap();
    std::fs::write(&ticket.path, b"abc").unwrap();
    q.finish("0", &new.generation, None).unwrap();
    assert!(q.snapshot()["tasks"].as_array().unwrap().is_empty());
    assert!(!q.control("0", "pause").unwrap());
    assert!(!q.finish("0", &new.generation, None).unwrap());
}
#[test]
fn changed_or_deleted_message_and_short_file_fail_without_import() {
    for mode in ["changed", "deleted", "short"] {
        let (_dir, db, q) = fixture(1);
        q.enqueue("message", "0", "peer").unwrap();
        let (task, ticket) = start(&q);
        std::fs::write(
            &ticket.path,
            if mode == "short" {
                b"ab".as_slice()
            } else {
                b"abc".as_slice()
            },
        )
        .unwrap();
        if mode == "deleted" {
            db.with_conn(|c| c.execute("DELETE FROM chats WHERE id='message'", []))
                .unwrap();
        }
        if mode == "changed" {
            db.with_conn(|c|c.execute("UPDATE chats SET content=json_set(content,'$.value.items[0].uri','fsid:changed') WHERE id='message'",[])).unwrap();
        }
        q.finish("0", &task.generation, None).unwrap();
        assert_eq!(status(&q, "0"), "FAILED");
        assert!(!std::path::Path::new(&ticket.path).exists());
        assert_eq!(
            db.with_conn(
                |c| c.query_row("SELECT count(*) FROM app_files", [], |r| r.get::<_, i64>(0))
            )
            .unwrap(),
            0
        );
    }
}
#[test]
fn remove_recreate_stale_generation_and_drop_clean_owned_temporary_files() {
    let (_dir, _db, q) = fixture(1);
    q.enqueue("message", "0", "peer").unwrap();
    let (old, old_ticket) = start(&q);
    assert!(q.control("0", "remove").unwrap());
    assert!(!std::path::Path::new(&old_ticket.path).exists());
    assert!(matches!(q.effect(), Some(Effect::Cancel { .. })));
    q.enqueue("message", "0", "peer").unwrap();
    let (new, ticket) = start(&q);
    assert_ne!(new.generation, old.generation);
    assert!(!q.finish("0", &old.generation, Some("late".into())).unwrap());
    assert!(std::path::Path::new(&ticket.path).exists());
    drop(q);
    assert!(!std::path::Path::new(&ticket.path).exists());
}
#[test]
fn queued_tasks_recheck_current_peer_and_attachment_before_start() {
    let (_dir, db, q) = fixture(4);
    for n in 0..4 {
        q.enqueue("message", &n.to_string(), "peer").unwrap();
    }
    let (first, _) = start(&q);
    start(&q);
    start(&q);
    db.with_conn(|c| c.execute("DELETE FROM peers WHERE id='peer'", []))
        .unwrap();
    q.finish(&first.id, &first.generation, Some("offline".into()))
        .unwrap();
    assert_eq!(status(&q, "3"), "FAILED");
    assert!(q.effect().is_none());
    let public = q.public_progress();
    assert!(!public.contains("peer"));
    assert!(!public.contains("generation"));
    assert!(public.contains("failed"));
}
#[test]
fn registry_and_effect_capacity_are_bounded() {
    let (_dir, _db, q) = fixture(129);
    for n in 0..128 {
        assert!(q.enqueue("message", &n.to_string(), "peer").unwrap());
    }
    assert!(q.enqueue("message", "128", "peer").is_err());
    let (_effects_dir, _effects_db, q) = fixture(1);
    q.enqueue("message", "0", "peer").unwrap();
    let mut count = 0;
    loop {
        if q.control("0", "remove").is_err() {
            break;
        }
        q.enqueue("message", "0", "peer").unwrap();
        count += 1;
        if count > 300 {
            panic!("unbounded effects");
        }
    }
    assert!(count <= 250);
}

#[test]
fn message_enqueue_and_control_use_one_owned_file_queue_and_one_revision_per_batch() {
    let (_dir, _db, queue) = fixture(4);
    assert!(queue.enqueue_message("message", "peer").unwrap());
    assert_eq!(queue.snapshot()["revision"], 1);
    assert_eq!(queue.snapshot()["tasks"].as_array().unwrap().len(), 4);
    assert!(!queue.enqueue_message("message", "peer").unwrap());
    assert_eq!(queue.snapshot()["revision"], 1);
    assert!(
        queue
            .control_message("message", Some("other"), "pause")
            .is_err()
    );
    assert_eq!(queue.snapshot()["revision"], 1);
    assert!(
        queue
            .control_message("message", Some("peer"), "pause")
            .unwrap()
    );
    assert_eq!(queue.snapshot()["revision"], 2);
    assert!(
        queue.snapshot()["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|task| task["status"] == "PAUSED")
    );
    assert!(
        queue
            .control_message("message", Some("peer"), "resume")
            .unwrap()
    );
    assert_eq!(queue.snapshot()["revision"], 3);
}

#[test]
fn oversized_message_enqueue_does_not_leave_partial_tasks_or_effects() {
    let (_dir, _db, queue) = fixture(129);
    assert!(queue.enqueue_message("message", "peer").is_err());
    assert_eq!(queue.snapshot()["tasks"], json!([]));
    assert_eq!(queue.snapshot()["revision"], 0);
    assert!(queue.effect().is_none());
}

#[test]
fn recreated_queue_has_distinct_runtime_identity_and_starts_at_revision_zero() {
    let (dir, db, first) = fixture(1);
    first.enqueue_message("message", "peer").unwrap();
    let restarted = Queue::new(db, dir.path().to_path_buf(), Arc::new(Imports::default()));
    assert_ne!(
        first.snapshot()["runtimeId"],
        restarted.snapshot()["runtimeId"]
    );
    assert_eq!(restarted.snapshot()["revision"], 0);
    assert_eq!(restarted.snapshot()["tasks"], json!([]));
}

#[test]
fn committed_downloads_release_all_queue_capacity_without_client_remove_requests() {
    let (dir, db, queue) = fixture(129);
    for id in 0..129 {
        let id = id.to_string();
        assert!(queue.enqueue("message", &id, "peer").unwrap());
        let (task, ticket) = start(&queue);
        std::fs::write(&ticket.path, b"abc").unwrap();
        assert!(queue.finish(&id, &task.generation, None).unwrap());
        assert!(queue.snapshot()["tasks"].as_array().unwrap().is_empty());
        assert_eq!(queue.public_progress(), "[]");
        assert!(!queue.progress(&id, &task.generation, 3).unwrap());
        assert!(!queue.finish(&id, &task.generation, None).unwrap());
    }
    let chat = messages::get(&db, "message").unwrap().unwrap();
    let content: Value = serde_json::from_str(&chat.content).unwrap();
    let items = content["value"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 129);
    for item in items {
        let uri = item["uri"].as_str().unwrap();
        assert!(uri.starts_with("fid:"));
        let id = uri[4..].split('.').next().unwrap();
        let file = db.app_file_get(id).unwrap().unwrap();
        assert_eq!(
            std::fs::read(dir.path().join(file.real_path)).unwrap(),
            b"abc"
        );
    }
}

#[test]
fn explicit_retry_intent_is_consumed_only_by_its_accepted_download_generation() {
    let (_dir, _db, queue) = fixture(2);
    assert!(queue.enqueue("message", "0", "peer").unwrap());
    let Some(Effect::Start { task, attempt, .. }) = queue.effect() else {
        panic!("Expected start")
    };
    assert_eq!(attempt, Attempt::Automatic);
    assert!(!queue.control("0", "retry").unwrap());
    queue
        .finish(&task.id, &task.generation, Some("offline".into()))
        .unwrap();
    assert!(queue.control("0", "retry").unwrap());
    let Some(Effect::Start {
        task: retried,
        attempt,
        ..
    }) = queue.effect()
    else {
        panic!("Expected retry start")
    };
    assert_eq!(attempt, Attempt::Retry);
    assert_ne!(retried.generation, task.generation);
    assert!(
        !queue
            .finish(&task.id, &task.generation, Some("late".into()))
            .unwrap()
    );
    assert_eq!(
        queue.state.lock().unwrap().tasks["0"].attempt,
        Attempt::Automatic
    );
    assert!(queue.enqueue("message", "1", "peer").unwrap());
    let Some(Effect::Start { attempt, .. }) = queue.effect() else {
        panic!("Expected neighbor start")
    };
    assert_eq!(attempt, Attempt::Automatic);
    assert!(
        queue.snapshot()["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|task| task.get("attempt").is_none())
    );
}

#[test]
fn message_retry_marks_both_existing_failed_tasks_and_fresh_fallback_tasks() {
    let (_dir, _db, queue) = fixture(1);
    assert!(queue.retry_message("message", "peer").unwrap());
    let Some(Effect::Start { task, attempt, .. }) = queue.effect() else {
        panic!("Expected start")
    };
    assert_eq!(attempt, Attempt::Retry);
    queue
        .finish(&task.id, &task.generation, Some("offline".into()))
        .unwrap();
    assert!(queue.retry_message("message", "peer").unwrap());
    let Some(Effect::Start { attempt, .. }) = queue.effect() else {
        panic!("Expected retry start")
    };
    assert_eq!(attempt, Attempt::Retry);
}
