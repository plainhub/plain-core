use super::*;

#[test]
fn archive_replaces_date_and_rejects_invalid_input_without_changing_existing_record() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("db.sqlite")).unwrap();
    archive(&db, "thread", "2026-10-05T01:00:00.000Z").unwrap();
    archive(&db, "thread", "2026-10-05T10:00:00.000+08:00").unwrap();
    let rows = db.archived_conversation_list().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].archived_at, "2026-10-05T02:00:00.000Z");
    assert!(archive(&db, "thread", "invalid").is_err());
    assert!(archive(&db, "", "2026-10-05T01:00:00.000Z").is_err());
    assert_eq!(
        db.archived_conversation_list().unwrap()[0].archived_at,
        rows[0].archived_at
    );
    assert_eq!(unarchive(&db, "thread").unwrap(), 1);
    assert_eq!(unarchive(&db, "thread").unwrap(), 0);
    assert!(db.archived_conversation_list().unwrap().is_empty());
}
