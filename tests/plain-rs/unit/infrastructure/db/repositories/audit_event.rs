use super::*;

#[test]
fn audit_filter_is_unicode_case_insensitive_before_pagination_and_sql_errors_are_real() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let older = db.audit_event_add("login", "ÄBC", Some(&plain_rs_client())).unwrap();
    let newer = db
        .audit_event_add("mount", "other", Some(&plain_rs_client()))
        .unwrap();
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE events SET created_at='2026-10-13T00:00:00.000Z' WHERE id=?1",
            [&older],
        )
    })
    .unwrap();
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE events SET created_at='2026-10-13T01:00:00.000Z' WHERE id=?1",
            [&newer],
        )
    })
    .unwrap();
    assert_eq!(db.audit_events(0, 1, None).unwrap()[0].id, newer);
    assert_eq!(db.audit_events(0, 1, Some("äbc")).unwrap()[0].id, older);
    assert!(db.audit_events(1, 1, Some("äbc")).unwrap().is_empty());
    db.with_conn(|conn| conn.execute_batch("DROP TABLE events"))
        .unwrap();
    assert!(db.audit_events(0, 1, None).is_err());
}

#[test]
fn audit_events_preserve_optional_client_ids() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let system_id = db.audit_event_add("mount", "/mnt/example", None).unwrap();
    let client_id = db
        .audit_event_add("update_device_name", "server", Some(&plain_rs_client()))
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
                "SELECT client_id IS NULL FROM events WHERE id=?1",
                [&system_id],
                |row| row.get(0),
            )
        })
        .unwrap();
    assert!(is_null);
}

fn plain_rs_client() -> crate::infrastructure::db::AuditEventClient {
    crate::infrastructure::db::AuditEventClient { client_id: "client".into(), ..Default::default() }
}

#[test]
fn audit_snapshots_survive_session_changes_and_search_every_client_field() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let mut session = crate::infrastructure::db::SessionRow {
        client_id: "fixture-client".into(), name: "Study computer".into(),
        client_ip: "192.0.2.24".into(), app_name: "Fixture browser".into(), app_version: "142".into(),
        os_name: "Windows".into(), os_version: "11".into(), url_token: "fixture-url-secret".into(),
        token: "fixture-session-secret".into(), r#type: crate::features::sessions::SessionType::Login,
        last_seen_at: None, created_at: "2026-01-01T00:00:00Z".into(), updated_at: "2026-01-01T00:00:00Z".into(),
    };
    db.session_save(&session).unwrap();
    let snapshot = db.audit_event_client(&session.client_id).unwrap();
    let id = db.audit_event_add("login", "", Some(&snapshot)).unwrap();
    session.name = "Changed name".into();
    session.client_ip = "192.0.2.25".into();
    db.session_save(&session).unwrap();
    db.session_delete(&session.client_id).unwrap();
    for needle in ["Study computer", "192.0.2.24", "WINDOWS", "11", "Fixture browser", "142", "fixture-client"] {
        let rows = db.audit_events(0, 10, Some(needle)).unwrap();
        assert_eq!(rows.len(), 1, "{needle}");
        assert_eq!(rows[0].id, id);
        assert_eq!(rows[0].client_name, "Study computer");
        assert_eq!(rows[0].client_ip, "192.0.2.24");
        assert!(db.audit_events(1, 10, Some(needle)).unwrap().is_empty());
    }
    for needle in ["Changed name", "192.0.2.25", "fixture-url-secret", "fixture-session-secret"] {
        assert!(db.audit_events(0, 10, Some(needle)).unwrap().is_empty(), "{needle}");
    }
}
