use crate::features::services::auth_error::AuthErrorCode;
use crate::features::services::disk_format_error::DiskFormatErrorCode;
use crate::infrastructure::platform::mount_error::MountErrorCode;
use super::*;
use crate::features::audit_events::*;
use crate::features::sessions::SessionType;

#[test]
fn audit_filter_is_unicode_case_insensitive_before_pagination_and_sql_errors_are_real() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let older = db
        .audit_event_add(
            &AuditEvent::UpdateDeviceName(DeviceNameEventData {
                old_name: "old".into(),
                new_name: "ÄBC".into(),
            }),
            Some(&plain_rs_client()),
            &plain_libs::utils::dbtime::now_iso(),
        )
        .unwrap();
    let newer = db
        .audit_event_add(
            &mount_event("other"),
            Some(&plain_rs_client()),
            &plain_libs::utils::dbtime::now_iso(),
        )
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
    let system_id = db
        .audit_event_add(
            &mount_event("/mnt/example"),
            None,
            &plain_libs::utils::dbtime::now_iso(),
        )
        .unwrap();
    let client_id = db
        .audit_event_add(
            &AuditEvent::UpdateDeviceName(DeviceNameEventData {
                old_name: "old".into(),
                new_name: "server".into(),
            }),
            Some(&plain_rs_client()),
            &plain_libs::utils::dbtime::now_iso(),
        )
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
        Some("706c6169-26be-890e-b404-adf1389070db")
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
    crate::infrastructure::db::AuditEventClient {
        client_id: "706c6169-26be-890e-b404-adf1389070db".into(),
        ..Default::default()
    }
}

#[test]
fn audit_snapshots_survive_session_changes_and_search_every_client_field() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let mut session = crate::infrastructure::db::SessionRow {
        client_id: "706c6169-b9ef-8b64-b37d-1162ef232835".into(),
        name: "Study computer".into(),
        client_ip: "192.0.2.24".into(),
        app_name: "Fixture browser".into(),
        app_version: "142".into(),
        os_name: "Windows".into(),
        os_version: "11".into(),
        url_token: "fixture-url-secret".into(),
        token: "fixture-session-secret".into(),
        r#type: crate::features::sessions::SessionType::Login,
        last_seen_at: None,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    };
    db.session_save(&session).unwrap();
    let snapshot = db.audit_event_client(&session.client_id).unwrap();
    let id = db
        .audit_event_add(
            &AuditEvent::Login(LoginEventData {
                session_type: SessionType::Login,
            }),
            Some(&snapshot),
            &plain_libs::utils::dbtime::now_iso(),
        )
        .unwrap();
    session.name = "Changed name".into();
    session.client_ip = "192.0.2.25".into();
    db.session_save(&session).unwrap();
    db.session_delete(&session.client_id).unwrap();
    for needle in [
        "Study computer",
        "192.0.2.24",
        "WINDOWS",
        "11",
        "Fixture browser",
        "142",
        "706c6169-b9ef-8b64-b37d-1162ef232835",
    ] {
        let rows = db.audit_events(0, 10, Some(needle)).unwrap();
        assert_eq!(rows.len(), 1, "{needle}");
        assert_eq!(rows[0].id, id);
        assert_eq!(rows[0].client_name.as_deref(), Some("Study computer"));
        assert_eq!(rows[0].client_ip.as_deref(), Some("192.0.2.24"));
        assert!(db.audit_events(1, 10, Some(needle)).unwrap().is_empty());
    }
    for needle in [
        "Changed name",
        "192.0.2.25",
        "fixture-url-secret",
        "fixture-session-secret",
    ] {
        assert!(
            db.audit_events(0, 10, Some(needle)).unwrap().is_empty(),
            "{needle}"
        );
    }
}

fn mount_event(path: &str) -> AuditEvent {
    AuditEvent::Mount(VolumeEventData {
        volume: EventVolume {
            id: None,
            name: None,
            device_path: None,
            mount_path: path.into(),
            file_system: None,
        },
    })
}

#[test]
fn every_event_roundtrips_its_exact_payload_and_rejects_unknown_contracts() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let volume = EventVolume {
        id: Some("fsuuid:generated".into()),
        name: Some("Volume".into()),
        device_path: Some("/dev/generated1".into()),
        mount_path: "/mnt/generated".into(),
        file_system: Some("ext4".into()),
    };
    let disk = EventDisk {
        id: Some("disk-generated".into()),
        name: Some("Disk".into()),
        path: "/dev/generated".into(),
        size_bytes: Some(crate::infrastructure::types::Long(64000000000)),
    };
    let session = SessionEventData {
        session: EventSession {
            client_id: "706c6169-15b2-8ac4-83ea-4e6b0c3a4b3c".into(),
            client_name: "Target".into(),
            r#type: SessionType::ApiToken,
        },
    };
    let events = [
        (
            "LOGIN",
            AuditEvent::Login(LoginEventData {
                session_type: SessionType::Login,
            }),
        ),
        (
            "LOGIN_FAILED",
            AuditEvent::LoginFailed(LoginFailedEventData {
                reason: AuthErrorCode::InvalidPassword,
            }),
        ),
        ("LOGOUT", AuditEvent::Logout(session.clone())),
        ("REVOKE_SESSION", AuditEvent::RevokeSession(session)),
        (
            "UPDATE_DEVICE_NAME",
            AuditEvent::UpdateDeviceName(DeviceNameEventData {
                old_name: "Old".into(),
                new_name: "New".into(),
            }),
        ),
        (
            "MOUNT",
            AuditEvent::Mount(VolumeEventData {
                volume: volume.clone(),
            }),
        ),
        (
            "MOUNT_FAILED",
            AuditEvent::MountFailed(VolumeFailedEventData {
                volume: volume.clone(),
                failure: MountFailure {
                    reason: MountErrorCode::MountFailed,
                    detail: None,
                },
            }),
        ),
        ("UNMOUNT", AuditEvent::Unmount(VolumeEventData { volume })),
        (
            "FORMAT_DISK",
            AuditEvent::FormatDisk(DiskFormatEventData {
                disk: disk.clone(),
                file_system: "ext4".into(),
            }),
        ),
        (
            "FORMAT_DISK_FAILED",
            AuditEvent::FormatDiskFailed(DiskFormatFailedEventData {
                disk,
                file_system: "ext4".into(),
                failure: DiskFormatFailure {
                    reason: DiskFormatErrorCode::FormatDiskFailed,
                    detail: None,
                },
            }),
        ),
    ];
    for (kind, event) in events {
        let id = db
            .audit_event_add(&event, None, &plain_libs::utils::dbtime::now_iso())
            .unwrap();
        let record = db.audit_events(0, 10, Some(&id)).unwrap().pop().unwrap();
        assert_eq!(record.r#type, event.kind());
        assert_eq!(record.data, event.clone().data());
        let (stored_kind, data, missing_actor): (String, String, bool) = db.with_conn(|conn| conn.query_row(
            "SELECT type,data,client_id IS NULL AND client_name IS NULL AND client_ip IS NULL AND os_name IS NULL AND os_version IS NULL AND app_name IS NULL AND app_version IS NULL FROM events WHERE id=?1", [&id],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
        )).unwrap();
        assert_eq!(stored_kind, kind);
        assert!(missing_actor);
        let data: Value = plain_libs::serde_json::from_str(&data).unwrap();
        assert!(!data.is_null());
        if kind == "REVOKE_SESSION" {
            assert_eq!(
                data,
                json!({"session":{"clientId":"706c6169-15b2-8ac4-83ea-4e6b0c3a4b3c","clientName":"Target","type":"API_TOKEN"}})
            );
        }
        if kind == "LOGIN_FAILED" {
            assert_eq!(data, json!({"reason":"INVALID_PASSWORD"}));
        }
    }
    assert_eq!(db.audit_events(0, 100, None).unwrap().len(), 10);
    assert!(
        db.audit_events(0, 10, Some("disk-generated"))
            .unwrap()
            .len()
            == 2
    );
    assert!(
        db.with_conn(|conn| conn.execute("UPDATE events SET data='invalid'", []))
            .is_err()
    );
    db.with_conn(|conn| conn.execute("UPDATE events SET type='UNKNOWN' WHERE type='LOGIN'", []))
        .unwrap();
    assert!(db.audit_events(0, 100, None).is_err());
    for payload in [
        json!({"type":"LOGIN_FAILED","data":{"reason":"unknown"}}),
        json!({"type":"REVOKE_SESSION","data":{"session":{"clientId":"706c6169-c03b-8906-9beb-520d0cf78a03","clientName":"X","type":"UNKNOWN"}}}),
    ] {
        assert!(plain_libs::serde_json::from_value::<AuditEvent>(payload).is_err());
    }
}

#[test]
fn payload_search_matches_decoded_strings_before_pagination() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let event = AuditEvent::UpdateDeviceName(DeviceNameEventData {
        old_name: "Old".into(),
        new_name: "Ä \"quoted\"\nname".into(),
    });
    let id = db
        .audit_event_add(&event, None, "2026-01-01T00:00:00.000Z")
        .unwrap();
    let rows = db.audit_events(0, 1, Some("ä \"quoted\"\nname")).unwrap();
    assert_eq!(rows[0].id, id);
    assert!(db.audit_events(1, 1, Some("quoted")).unwrap().is_empty());
    assert!(db.audit_events(0, 10, Some("newName")).unwrap().is_empty());
}
