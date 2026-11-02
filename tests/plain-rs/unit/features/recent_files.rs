use super::*;

fn recent() -> (tempfile::TempDir, Arc<Db>, RecentFiles) {
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(Db::open(&root.path().join("plain.db")).unwrap());
    let recent = RecentFiles::new(store.clone());
    (root, store, recent)
}

#[test]
fn fifty_memory_records_are_visible_before_a_single_fifty_first_record_commit() {
    let (_root, store, recent) = recent();
    let before = store.write_attempts_for_test();
    for n in 0..50 {
        recent.add(&format!("file-{n}"));
    }
    assert_eq!(store.write_attempts_for_test(), before);
    assert!(store.get(KEY).unwrap().is_none());
    assert_eq!(recent.list(0).unwrap().len(), 50);
    assert_eq!(recent.list(1).unwrap(), ["file-49"]);
    recent.add("file-50");
    assert_eq!(store.write_attempts_for_test(), before);
    assert!(recent.flush_due(Instant::now()).unwrap());
    assert_eq!(store.write_attempts_for_test(), before + 1);
    assert_eq!(
        store.get_json::<Vec<String>>(KEY).unwrap().unwrap().len(),
        51
    );
    assert!(!recent.flush().unwrap());
}

#[test]
fn queries_merge_memory_with_stored_paths_and_deduplicate_newest_first() {
    let (_root, store, recent) = recent();
    store.insert(KEY, r#"["shared","old"]"#).unwrap();
    let before = store.write_attempts_for_test();
    recent.add("new");
    recent.add("shared");
    assert_eq!(recent.list(0).unwrap(), ["shared", "new", "old"]);
    assert_eq!(recent.list(2).unwrap(), ["shared", "new"]);
    assert_eq!(store.write_attempts_for_test(), before);
    recent.flush().unwrap();
    assert_eq!(
        store.get_json::<Vec<String>>(KEY).unwrap().unwrap(),
        ["shared", "new", "old"]
    );
}

#[test]
fn one_minute_flush_and_failed_commit_keep_pending_records_visible() {
    let (_root, store, recent) = recent();
    let now = Instant::now();
    recent.pending.lock().unwrap().push("new".into(), now);
    let before = store.write_attempts_for_test();
    assert!(
        !recent
            .flush_due(now + std::time::Duration::from_secs(59))
            .unwrap()
    );
    assert_eq!(store.write_attempts_for_test(), before);
    store.set_write_failure_for_test(true);
    assert!(
        recent
            .flush_due(now + std::time::Duration::from_secs(60))
            .is_err()
    );
    assert_eq!(recent.list(0).unwrap(), ["new"]);
    assert!(store.get(KEY).unwrap().is_none());
    store.set_write_failure_for_test(false);
    assert!(recent.flush().unwrap());
    assert_eq!(
        store.get_json::<Vec<String>>(KEY).unwrap().unwrap(),
        ["new"]
    );
    assert!(
        !recent
            .flush_due(now + std::time::Duration::from_secs(120))
            .unwrap()
    );
}

#[test]
fn merged_history_keeps_only_the_five_hundred_most_recent_unique_paths() {
    let (_root, _store, recent) = recent();
    for n in 0..550 {
        recent.add(&format!("file-{n}"));
    }
    recent.add("file-100");
    let values = recent.list(0).unwrap();
    assert_eq!(values.len(), 500);
    assert_eq!(values[0], "file-100");
    assert_eq!(values[1], "file-549");
    assert_eq!(
        values.iter().filter(|p| p.as_str() == "file-100").count(),
        1
    );
    recent.flush().unwrap();
    assert_eq!(recent.list(0).unwrap(), values);
}
