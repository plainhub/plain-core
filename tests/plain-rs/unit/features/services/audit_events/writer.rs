use super::*;
use crate::features::audit_events::{DeviceNameEventData, EventData, EventType};

fn rename(old_name: &str, new_name: &str) -> AuditEvent {
    AuditEvent::UpdateDeviceName(DeviceNameEventData {
        old_name: old_name.into(),
        new_name: new_name.into(),
    })
}

#[tokio::test]
async fn submission_never_waits_for_sqlite_and_flush_drains_owned_snapshots() {
    let db = Arc::new(Db::open(std::path::Path::new(":memory:")).unwrap());
    let writer = AuditEvents::start(db.clone()).unwrap();
    let mut actor = AuditEventClient {
        client_id: "706c6169-5847-8f52-9073-afe9de5f9211".into(),
        client_name: "Before".into(),
        ..Default::default()
    };
    db.with_conn(|conn| {
        writer
            .record(rename("first", "second"), Some(actor.clone()))
            .unwrap();
        writer
            .record(rename("second", "third"), Some(actor.clone()))
            .unwrap();
        actor.client_name = "After".into();
        assert_eq!(
            conn.query_row("SELECT count(*) FROM events", [], |row| row
                .get::<_, i64>(0))?,
            0
        );
        Ok::<(), plain_libs::rusqlite::Error>(())
    })
    .unwrap();
    writer.flush().await.unwrap();
    let records = db.audit_events(0, 10, None).unwrap();
    assert_eq!(records.len(), 2);
    assert!(
        records
            .iter()
            .all(|row| row.r#type == EventType::UpdateDeviceName
                && row.client_name.as_deref() == Some("Before"))
    );
    for target in ["second", "third"] {
        assert!(records.iter().any(
            |row| matches!(&row.data, EventData::DeviceName(data) if data.new_name == target)
        ));
    }
    writer.record(rename("third", "fourth"), None).unwrap();
    writer.shutdown().await.unwrap();
    assert_eq!(db.audit_events(0, 10, None).unwrap().len(), 3);
    assert!(writer.record(rename("fourth", "fifth"), None).is_err());
}

#[tokio::test]
async fn writer_reports_database_errors_through_the_completion_barrier() {
    let db = Arc::new(Db::open(std::path::Path::new(":memory:")).unwrap());
    let writer = AuditEvents::start(db.clone()).unwrap();
    db.with_conn(|conn| conn.execute_batch("DROP TABLE events"))
        .unwrap();
    writer.record(rename("old", "new"), None).unwrap();
    assert!(writer.flush().await.unwrap_err().contains("events"));
    writer.shutdown().await.unwrap();
}
