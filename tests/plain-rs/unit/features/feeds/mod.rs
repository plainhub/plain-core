use super::*;

#[test]
fn editing_fields_is_one_atomic_update_and_partial_patches_preserve_other_fields() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    db.feed_save(
        "one",
        "Original",
        "https://example.org/one",
        true,
        "2026-01-01T00:00:00Z",
    )
    .unwrap();
    db.with_conn(|connection| connection.execute_batch("CREATE TEMP TABLE feed_write_events(id TEXT); CREATE TEMP TRIGGER observe_feed_write AFTER UPDATE ON main.feeds BEGIN INSERT INTO feed_write_events VALUES(new.id); END;")).unwrap();
    let writes = || {
        db.with_conn(|connection| {
            connection.query_row("SELECT COUNT(*) FROM feed_write_events", [], |row| {
                row.get::<_, i64>(0)
            })
        })
        .unwrap()
    };
    let feed = update_fields(
        &db,
        "one",
        Some("Renamed"),
        Some("https://example.org/new"),
        Some(false),
    )
    .unwrap();
    assert_eq!(writes(), 1);
    assert_eq!(
        (&*feed.name, &*feed.url, feed.fetch_full_content),
        ("Renamed", "https://example.org/new", false)
    );
    let feed = update_fields(&db, "one", None, None, Some(true)).unwrap();
    assert_eq!(writes(), 2);
    assert_eq!(
        (&*feed.name, &*feed.url, feed.fetch_full_content),
        ("Renamed", "https://example.org/new", true)
    );
}

#[test]
fn invalid_duplicate_empty_and_missing_updates_preserve_existing_rows() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    db.feed_save(
        "one",
        "Original",
        "https://example.org/one",
        false,
        "2026-01-01T00:00:00Z",
    )
    .unwrap();
    db.feed_save(
        "neighbor",
        "Neighbor",
        "https://example.org/neighbor",
        true,
        "2026-01-01T00:00:00Z",
    )
    .unwrap();
    for (url, expected) in [
        ("file:///fixture/feed", "invalid_feed_url"),
        ("https://example.org/neighbor", "feed_already_exists"),
    ] {
        assert!(
            update_fields(&db, "one", Some("Must not persist"), Some(url), Some(true))
                .unwrap_err()
                .to_string()
                .contains(expected)
        );
        let feed = get(&db, "one").unwrap().unwrap();
        assert_eq!(
            (&*feed.name, &*feed.url, feed.fetch_full_content),
            ("Original", "https://example.org/one", false)
        );
    }
    assert!(
        update_fields(&db, "one", None, None, None)
            .unwrap_err()
            .to_string()
            .contains("feed_update_requires_changes")
    );
    assert!(
        update_fields(&db, "missing", Some("Name"), None, None)
            .unwrap_err()
            .to_string()
            .contains("feed_not_found")
    );
    let neighbor = get(&db, "neighbor").unwrap().unwrap();
    assert_eq!(
        (&*neighbor.name, &*neighbor.url, neighbor.fetch_full_content),
        ("Neighbor", "https://example.org/neighbor", true)
    );
    assert_eq!(list(&db).unwrap().len(), 2);
}
