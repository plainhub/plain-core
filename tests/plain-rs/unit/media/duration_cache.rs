use super::*;

fn value(kind: &str, id: &str, path: &str, duration_ms: i64) -> Duration {
    Duration {
        media_type: kind.into(),
        file_id: id.into(),
        path: path.into(),
        size_bytes: 42,
        modified_at: "2026-01-01T00:00:00Z".into(),
        duration_ms,
        updated_at: String::new(),
    }
}

#[test]
fn durations_survive_reopening_and_keep_exact_units_and_typed_ids() {
    let directory = tempfile::tempdir().unwrap();
    {
        let cache = DurationCache::new(directory.path());
        cache
            .save(value("audio", "same", "/audio", 5_000_000_001))
            .unwrap();
        cache
            .save(value("video", "same", "/video", 6_000_000_001))
            .unwrap();
    }
    let cache = DurationCache::new(directory.path());
    let rows = cache.all().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].duration_ms, 5_000_000_001);
    assert_eq!(rows[0].path, "/audio");
    assert_eq!(rows[0].size_bytes, 42);
    assert_eq!(rows[0].modified_at, "2026-01-01T00:00:00Z");
    assert!(!rows[0].updated_at.is_empty());
    assert_eq!(
        cache
            .remove("audio", &["same".into(), "same".into()])
            .unwrap(),
        1
    );
    assert_eq!(cache.all().unwrap()[0].duration_ms, 6_000_000_001);
    assert_eq!(cache.remove("audio", &["missing".into()]).unwrap(), 0);
    assert_eq!(cache.remove("video", &[]).unwrap(), 0);
}

#[test]
fn path_invalidation_respects_directory_boundaries_and_other_prefixes() {
    let directory = tempfile::tempdir().unwrap();
    let cache = DurationCache::new(directory.path());
    cache
        .save(value("audio", "removed", "/root/song", 10))
        .unwrap();
    cache
        .save(value("video", "nested", "/root/nested/video", 20))
        .unwrap();
    cache
        .save(value("audio", "sibling", "/root_sibling/song", 30))
        .unwrap();
    cache
        .save(value("video", "failed", "/other/failed", 40))
        .unwrap();
    let db = Db::open(&crate::data_paths::DataPaths::new(directory.path()).database()).unwrap();
    db.insert("session:keep", "not a duration").unwrap();
    cache.invalidate_paths(&["/root/song".into()], &[]).unwrap();
    assert_eq!(cache.all().unwrap().len(), 3);
    cache.invalidate_paths(&[], &["/root".into()]).unwrap();
    assert_eq!(cache.all().unwrap().len(), 2);
    assert_eq!(
        db.get("session:keep").unwrap().as_deref(),
        Some("not a duration")
    );
}

#[test]
fn invalid_entries_do_not_replace_valid_cache() {
    let directory = tempfile::tempdir().unwrap();
    let cache = DurationCache::new(directory.path());
    cache.save(value("audio", "one", "/file", 100)).unwrap();
    for invalid in [
        value("image", "one", "/file", 200),
        value("audio", "", "/file", 200),
        value("audio", "one", "", 200),
        value("audio", "one", "/file", 0),
        value("audio", "one", "/file", -1),
    ] {
        assert!(cache.save(invalid).is_err());
    }
    let mut invalid = value("audio", "one", "/file", 200);
    invalid.modified_at = "invalid".into();
    assert!(cache.save(invalid).is_err());
    assert!(cache.remove("image", &["one".into()]).is_err());
    assert_eq!(cache.all().unwrap()[0].duration_ms, 100);
}

#[test]
fn malformed_cache_is_reported_and_open_failure_is_not_an_empty_list() {
    let directory = tempfile::tempdir().unwrap();
    let db = Db::open(&crate::data_paths::DataPaths::new(directory.path()).database()).unwrap();
    db.insert(format!("{PREFIX}audio:bad"), "invalid JSON")
        .unwrap();
    assert!(DurationCache::new(directory.path()).all().is_err());
    let blocked = tempfile::tempdir().unwrap();
    std::fs::write(
        crate::data_paths::DataPaths::new(blocked.path()).database(),
        b"file",
    )
    .unwrap();
    assert!(DurationCache::new(blocked.path()).all().is_err());
}

#[test]
fn fresh_sqlite_schema_has_no_duration_cache_table() {
    let db = crate::db::Db::open(Path::new(":memory:")).unwrap();
    let exists = db
        .with_conn(|conn| {
            conn.query_row(
                "SELECT count(*) FROM sqlite_master WHERE name='media_item'",
                [],
                |row| row.get::<_, i64>(0),
            )
        })
        .unwrap();
    assert_eq!(exists, 0);
}

#[test]
fn matching_requires_the_same_kind_path_size_and_provider_second() {
    let directory = tempfile::tempdir().unwrap();
    let cache = DurationCache::new(directory.path());
    cache
        .save(value("video", "same", "/video", 5_000_000_001))
        .unwrap();
    let item = Identity {
        media_type: "video".into(),
        file_id: "same".into(),
        path: "/video".into(),
        size_bytes: 42,
        modified_at: "2026-01-01T00:00:00.999Z".into(),
    };
    let mut wrong_kind = item.clone();
    wrong_kind.media_type = "audio".into();
    let mut wrong_path = item.clone();
    wrong_path.path = "/replaced".into();
    let mut wrong_size = item.clone();
    wrong_size.size_bytes = 43;
    let mut wrong_time = item.clone();
    wrong_time.modified_at = "2026-01-01T00:00:01Z".into();
    assert_eq!(
        cache
            .matching(&[item, wrong_kind, wrong_path, wrong_size, wrong_time])
            .unwrap(),
        [Some(5_000_000_001), None, None, None, None]
    );
}

#[test]
fn removal_and_path_invalidation_prevent_inflight_probes_from_resurrecting_entries() {
    let directory = tempfile::tempdir().unwrap();
    let cache = DurationCache::new(directory.path());
    let first = cache.revision();
    cache.remove("audio", &["deleted".into()]).unwrap();
    assert!(
        !cache
            .save_at_revision(value("audio", "deleted", "/gone", 5000), first)
            .unwrap()
    );
    let second = cache.revision();
    cache.invalidate_paths(&["/changed".into()], &[]).unwrap();
    assert!(
        !cache
            .save_at_revision(value("audio", "moved", "/changed", 6000), second)
            .unwrap()
    );
    assert!(cache.all().unwrap().is_empty());
    assert!(
        cache
            .save_at_revision(
                value("audio", "current", "/present", 7000),
                cache.revision()
            )
            .unwrap()
    );
    assert_eq!(cache.all().unwrap()[0].duration_ms, 7000);
}

#[test]
fn unrelated_removals_cancel_pending_probes_without_committing_or_flushing() {
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(LazyDb::new(
        crate::data_paths::DataPaths::new(directory.path()).database(),
    ));
    let cache = DurationCache::with_store(store.clone());
    cache
        .save(value("audio", "kept", "/kept/song", 5000))
        .unwrap();
    let db = store.get().unwrap();
    let before = db.get(key("audio", "kept").unwrap()).unwrap().unwrap();
    let revision = cache.revision();
    let writes = db.write_attempts_for_test();
    db.set_write_failure_for_test(true);

    assert_eq!(cache.remove("audio", &["missing".into()]).unwrap(), 0);
    cache
        .invalidate_paths(&["/unrelated".into()], &["/other".into()])
        .unwrap();
    assert_eq!(cache.revision(), revision + 2);
    assert!(
        !cache
            .save_at_revision(value("audio", "stale", "/unrelated", 6000), revision)
            .unwrap()
    );
    assert_eq!(db.write_attempts_for_test(), writes);
    assert_eq!(
        db.get(key("audio", "kept").unwrap()).unwrap().unwrap(),
        before
    );
    assert!(cache.invalidate_paths(&[], &["/kept".into()]).is_err());
    assert_eq!(db.write_attempts_for_test(), writes + 1);
    assert_eq!(
        db.get(key("audio", "kept").unwrap()).unwrap().unwrap(),
        before
    );

    db.set_write_failure_for_test(false);
    cache.invalidate_paths(&[], &["/kept".into()]).unwrap();
    assert_eq!(db.write_attempts_for_test(), writes + 2);
    assert!(cache.all().unwrap().is_empty());
}
