use super::*;

#[test]
fn audit_filter_is_unicode_case_insensitive_before_pagination_and_sql_errors_are_real() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let older = db.audit_event_add("login", "ÄBC", Some("client")).unwrap();
    let newer = db
        .audit_event_add("mount", "other", Some("client"))
        .unwrap();
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE audit_events SET created_at='2026-10-13T00:00:00.000Z' WHERE id=?1",
            [&older],
        )
    })
    .unwrap();
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE audit_events SET created_at='2026-10-13T01:00:00.000Z' WHERE id=?1",
            [&newer],
        )
    })
    .unwrap();
    assert_eq!(db.audit_events(0, 1, None).unwrap()[0].id, newer);
    assert_eq!(db.audit_events(0, 1, Some("äbc")).unwrap()[0].id, older);
    assert!(db.audit_events(1, 1, Some("äbc")).unwrap().is_empty());
    db.with_conn(|conn| conn.execute_batch("DROP TABLE audit_events"))
        .unwrap();
    assert!(db.audit_events(0, 1, None).is_err());
}

#[test]
fn audit_events_preserve_optional_client_ids() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let system_id = db.audit_event_add("mount", "/mnt/example", None).unwrap();
    let client_id = db
        .audit_event_add("set_hostname", "server", Some("client"))
        .unwrap();
    let rows = db.audit_events(0, 10, None).unwrap();
    assert_eq!(
        rows.iter()
            .find(|row| row.id == system_id)
            .unwrap()
            .client_id,
        None
    );
    assert_eq!(
        rows.iter()
            .find(|row| row.id == client_id)
            .unwrap()
            .client_id
            .as_deref(),
        Some("client")
    );
    let is_null: bool = db
        .with_conn(|conn| {
            conn.query_row(
                "SELECT client_id IS NULL FROM audit_events WHERE id=?1",
                [&system_id],
                |row| row.get(0),
            )
        })
        .unwrap();
    assert!(is_null);
}
