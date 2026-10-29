use super::*;

fn fixture() -> (tempfile::TempDir, Db) {
    let root = tempfile::tempdir().unwrap();
    let database = Db::open(&root.path().join("plain.db")).unwrap();
    (root, database)
}

#[test]
fn text_roundtrip_and_literal_unicode_prefixes_use_ordered_keys() {
    let (_root, database) = fixture();
    let keys = [
        "tag:a",
        "tag:b",
        "other:c",
        "中文:甲",
        "中文:乙",
        "a%_:one",
        "a00:two",
        "nul\0:one",
        "nul:two",
        "\u{10ffff}:one",
        "\u{d7ff}:edge",
    ];
    for key in keys {
        database.insert(key, "字符串 \0 value").unwrap();
    }
    for prefix in [
        "",
        "tag:",
        "中文:",
        "a%_:",
        "nul\0:",
        "\u{10ffff}",
        "\u{d7ff}",
        "absent",
    ] {
        let found = database
            .scan_prefix(prefix)
            .map(|row| row.unwrap().0)
            .collect::<Vec<_>>();
        let mut expected = keys
            .iter()
            .filter(|key| key.starts_with(prefix))
            .map(|key| key.to_string())
            .collect::<Vec<_>>();
        expected.sort();
        assert_eq!(found, expected, "{prefix:?}");
    }
    assert_eq!(
        database.get("中文:甲").unwrap().as_deref(),
        Some("字符串 \0 value")
    );
    database.remove("中文:甲").unwrap();
    assert_eq!(database.get("中文:甲").unwrap(), None);
}

#[test]
fn strict_table_rejects_blob_keys_and_values() {
    let (_root, database) = fixture();
    let connection = database.0.connection.lock().unwrap();
    assert!(
        connection
            .execute(
                "INSERT INTO key_values VALUES(?1,?2)",
                params![vec![1u8], "text"]
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "INSERT INTO key_values VALUES(?1,?2)",
                params!["text", vec![1u8]]
            )
            .is_err()
    );
    let count: i64 = connection
        .query_row("SELECT count(*) FROM key_values", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn failed_batch_rolls_back_every_earlier_statement() {
    let (_root, database) = fixture();
    database.insert("original", "keep").unwrap();
    database.0.connection.lock().unwrap().execute_batch("CREATE TRIGGER fail_write BEFORE INSERT ON key_values WHEN new.key='fail' BEGIN SELECT RAISE(ABORT,'injected write rejection'); END;").unwrap();
    let mut batch = database.batch();
    batch.remove("original");
    batch.insert("first", "new");
    batch.insert("fail", "reject");
    assert!(database.apply_batch(batch).is_err());
    assert_eq!(database.get("original").unwrap().as_deref(), Some("keep"));
    assert_eq!(database.get("first").unwrap(), None);
}

#[test]
fn batch_stages_changes_until_committed_and_can_apply_after_reopen() {
    let (root, database) = fixture();
    database.insert("one", "before").unwrap();
    let mut batch = database.batch_with_capacity(2);
    batch.remove("one");
    batch.insert("two", "after");
    assert_eq!(database.get("one").unwrap().as_deref(), Some("before"));
    assert_eq!(database.get("two").unwrap(), None);
    drop(database);
    let database = Db::open(&root.path().join("plain.db")).unwrap();
    database.apply_batch(batch).unwrap();
    assert_eq!(database.get("one").unwrap(), None);
    assert_eq!(database.get("two").unwrap().as_deref(), Some("after"));
}

#[test]
fn json_read_modify_write_is_atomic_across_connections() {
    let (root, first) = fixture();
    let second = Db::from_connection(Arc::new(Mutex::new(
        crate::sqlite::open(&root.path().join("plain.db")).unwrap(),
    )));
    let workers = [first.clone(), second]
        .into_iter()
        .map(|database| {
            std::thread::spawn(move || {
                for _ in 0..100 {
                    database
                        .update_json("counter", |value: &mut u64| {
                            *value += 1;
                            Ok(())
                        })
                        .unwrap();
                }
            })
        })
        .collect::<Vec<_>>();
    for worker in workers {
        worker.join().unwrap();
    }
    assert_eq!(first.get_json::<u64>("counter").unwrap(), Some(200));
}

#[test]
fn failed_json_callbacks_and_invalid_prefix_writes_preserve_original_values() {
    let (_root, database) = fixture();
    database.set_json("state:one", 1).unwrap();
    let result: Result<()> = database.update_json("state:one", |value: &mut u64| {
        *value = 2;
        anyhow::bail!("rejected operation")
    });
    assert!(result.is_err());
    assert!(
        database
            .update_json_prefix("state:", |map| {
                map.insert("outside".into(), serde_json::json!(3));
                map.remove("state:one");
                Ok(())
            })
            .is_err()
    );
    assert_eq!(database.get_json::<u64>("state:one").unwrap(), Some(1));
    assert_eq!(database.get("outside").unwrap(), None);
}

#[test]
fn unchanged_json_and_prefix_updates_do_not_write() {
    let (_root, database) = fixture();
    assert!(database.set_json("state:one", 1).unwrap());
    let before = database.write_attempts_for_test();
    assert!(!database.set_json("state:one", 1).unwrap());
    database.update_json_prefix("state:", |_| Ok(())).unwrap();
    assert_eq!(database.write_attempts_for_test(), before);
}

#[test]
fn prefix_updates_commit_deletions_and_multiple_values_together() {
    let (_root, database) = fixture();
    database.set_json("state:one", 1).unwrap();
    database.set_json("other:one", 2).unwrap();
    database
        .update_json_prefix("state:", |map| {
            map.remove("state:one");
            map.insert("state:two".into(), serde_json::json!({"value":3}));
            Ok(())
        })
        .unwrap();
    assert_eq!(database.get("state:one").unwrap(), None);
    assert_eq!(
        database.get_json::<serde_json::Value>("state:two").unwrap(),
        Some(serde_json::json!({"value":3}))
    );
    assert_eq!(database.get_json::<u64>("other:one").unwrap(), Some(2));
}

#[test]
fn injected_failure_keeps_batch_and_json_writes_uncommitted() {
    let (_root, database) = fixture();
    database.insert("one", "keep").unwrap();
    database.set_write_failure_for_test(true);
    let mut batch = database.batch();
    batch.remove("one");
    batch.insert("two", "new");
    assert!(database.apply_batch(batch).is_err());
    assert!(database.set_json("three", 3).is_err());
    assert_eq!(database.get("one").unwrap().as_deref(), Some("keep"));
    assert_eq!(database.get("two").unwrap(), None);
    assert_eq!(database.get("three").unwrap(), None);
}

#[test]
fn lazy_open_creates_a_database_file_only_on_first_use_and_shares_its_handle() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("plain.db");
    let lazy = LazyDb::new(path.clone());
    assert!(!path.exists());
    let first = lazy.get().unwrap();
    first.insert("key", "value").unwrap();
    assert!(path.is_file());
    let second = lazy.get().unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(second.get("key").unwrap().as_deref(), Some("value"));
}
