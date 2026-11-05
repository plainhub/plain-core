use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn temporary() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("plain-db-backup-")
        .tempdir()
        .unwrap()
}
#[test]
fn snapshot_contains_committed_wal_data_without_copying_sidecars() {
    let directory = temporary();
    let root = directory.path();
    let source = root.join("source.db");
    let target = root.join("snapshot.db");
    let db = Db::open(&source).unwrap();
    db.with_conn(|conn| conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; CREATE TABLE backup_values (id INTEGER PRIMARY KEY, value TEXT); INSERT INTO backup_values VALUES (1,'committed');").unwrap());
    db.backup_to(&target, || false).unwrap();
    let copy = Connection::open(&target).unwrap();
    assert_eq!(
        copy.query_row("SELECT value FROM backup_values WHERE id=1", [], |row| {
            row.get::<_, String>(0)
        })
        .unwrap(),
        "committed"
    );
    assert_eq!(
        copy.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    db.with_conn(|conn| {
        conn.execute("UPDATE backup_values SET value='later'", [])
            .unwrap()
    });
    assert_eq!(
        copy.query_row("SELECT value FROM backup_values WHERE id=1", [], |row| {
            row.get::<_, String>(0)
        })
        .unwrap(),
        "committed"
    );
    drop(copy);
    drop(db);
}
#[test]
fn snapshot_never_overwrites_existing_paths_or_the_live_database() {
    let directory = temporary();
    let root = directory.path();
    let source = root.join("source.db");
    let existing = root.join("existing.db");
    let db = Db::open(&source).unwrap();
    std::fs::write(&existing, b"keep").unwrap();
    assert!(db.backup_to(&existing, || false).is_err());
    assert_eq!(std::fs::read(&existing).unwrap(), b"keep");
    assert!(db.backup_to(&source, || false).is_err());
    assert!(source.exists());
    drop(db);
}
#[test]
fn cancellation_removes_only_the_new_snapshot() {
    let directory = temporary();
    let root = directory.path();
    let db = Db::open(&root.join("source.db")).unwrap();
    let target = root.join("cancelled.db");
    let samples = AtomicUsize::new(0);
    assert!(
        db.backup_to(&target, || samples.fetch_add(1, Ordering::SeqCst) > 0)
            .is_err()
    );
    assert!(!target.exists());
    assert!(root.join("source.db").exists());
    drop(db);
}
