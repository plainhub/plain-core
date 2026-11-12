use super::*;
fn fixture() -> (plain_libs::temporary::TempDir, Db) {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let db = Db::open(&dir.path().join("db.sqlite")).unwrap();
    (dir, db)
}
#[test]
fn receipts_survive_reopen_and_separate_clients_and_purposes() {
    let (dir, db) = fixture();
    assert!(
        db.admit_request("main", "a", 100000, "nonce", 100000, 60000)
            .unwrap()
    );
    drop(db);
    let db = Db::open(&dir.path().join("db.sqlite")).unwrap();
    assert!(
        !db.admit_request("main", "a", 100000, "nonce", 100000, 60000)
            .unwrap()
    );
    assert!(
        db.admit_request("main", "b", 100000, "nonce", 100000, 60000)
            .unwrap()
    );
    assert!(
        db.admit_request("peer", "a", 100000, "nonce", 100000, 60000)
            .unwrap()
    );
}
#[test]
fn independent_connections_claim_a_request_only_once() {
    let (dir, first) = fixture();
    let second = Db::open(&dir.path().join("db.sqlite")).unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let workers = (0..8)
        .map(|index| {
            let db = if index % 2 == 0 {
                first.clone()
            } else {
                second.clone()
            };
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                db.admit_request("main", "a", 100000, "nonce", 100000, 60000)
                    .unwrap()
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        workers
            .into_iter()
            .map(|w| w.join().unwrap())
            .filter(|accepted| *accepted)
            .count(),
        1
    );
}
#[test]
fn inclusive_future_boundary_and_clock_rollback_cannot_revive_cleaned_receipts() {
    let (dir, db) = fixture();
    assert!(
        db.admit_request("main", "a", 160000, "future", 100000, 60000)
            .unwrap()
    );
    assert!(
        !db.admit_request("main", "a", 160001, "too-future", 100000, 60000)
            .unwrap()
    );
    assert!(
        db.admit_request("main", "a", 220000, "advance", 220000, 60000)
            .unwrap()
    );
    assert!(
        !db.admit_request("main", "a", 160000, "future", 220000, 60000)
            .unwrap()
    );
    let count: i64 = db
        .with_conn(|c| c.query_row("SELECT COUNT(*) FROM replay_receipts", [], |r| r.get(0)))
        .unwrap();
    assert_eq!(count, 2);
    assert!(
        db.admit_request("main", "a", 220001, "clean", 220001, 60000)
            .unwrap()
    );
    let count: i64 = db
        .with_conn(|c| c.query_row("SELECT COUNT(*) FROM replay_receipts", [], |r| r.get(0)))
        .unwrap();
    assert_eq!(count, 2);
    drop(db);
    let db = Db::open(&dir.path().join("db.sqlite")).unwrap();
    assert!(
        !db.admit_request("main", "a", 160000, "future", 160000, 60000)
            .unwrap()
    );
    assert!(
        db.admit_request("main", "a", 220001, "reordered", 220000, 60000)
            .unwrap()
    );
}
#[test]
fn storage_read_and_write_failures_do_not_admit_requests() {
    let (_dir, db) = fixture();
    db.with_conn(|c| c.execute_batch("PRAGMA query_only=ON;"))
        .unwrap();
    assert!(
        db.admit_request("main", "a", 100000, "nonce", 100000, 60000)
            .is_err()
    );
    db.with_conn(|c| c.execute_batch("PRAGMA query_only=OFF;"))
        .unwrap();
    assert!(
        db.admit_request("main", "a", 100000, "nonce", 100000, 60000)
            .unwrap()
    );
    db.with_conn(|c| c.execute_batch("DROP TABLE replay_receipts;"))
        .unwrap();
    assert!(
        db.admit_request("main", "a", 100000, "other", 100000, 60000)
            .is_err()
    );
}
#[test]
fn development_database_tools_cannot_read_or_erase_security_receipts() {
    use crate::infrastructure::db::devtools;
    let (dir, db) = fixture();
    assert!(
        db.admit_request("main", "a", 100000, "nonce", 100000, 60000)
            .unwrap()
    );
    assert!(!devtools::tables(&db).iter().any(|t| t == "replay_receipts"));
    assert!(devtools::table_rows(&db, "replay_receipts", 0, 10).is_err());
    assert!(devtools::delete_table_rows(&db, "REPLAY_RECEIPTS", &["x".into()]).is_err());
    let file = devtools::SqliteFile::system(dir.path().join("db.sqlite"));
    assert!(
        !file
            .table_schemas()
            .unwrap()
            .iter()
            .any(|t| t.name == "replay_receipts")
    );
    assert!(file.table_rows_text("replay_receipts", 0, 10).is_err());
    assert!(
        file.delete_table_rows("replay_receipts", &["x".into()])
            .is_err()
    );
    assert!(
        file.insert_table_row("replay_receipts", &plain_libs::serde_json::Map::new())
            .is_err()
    );
    assert!(
        !db.admit_request("main", "a", 100000, "nonce", 100000, 60000)
            .unwrap()
    );
    let user_file = devtools::SqliteFile::new(dir.path().join("db.sqlite"));
    assert!(
        user_file
            .table_schemas()
            .unwrap()
            .iter()
            .any(|t| t.name == "replay_receipts")
    );
}
