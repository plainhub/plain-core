use crate::chat::enums::DeviceType;
use crate::db::chat_store::{SaveMode, peers};
use crate::db::{DPeer, Db};

#[test]
fn peer_changes_cover_repository_raw_sql_and_cloned_handles_without_other_tables() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let mut changes = db.peer_changes();
    db.with_conn(|connection| {
        connection.execute("UPDATE peers SET name='none' WHERE id='missing'", [])
    })
    .unwrap();
    assert!(!changes.has_changed().unwrap());
    let peer = DPeer::new("remote", "remote", "", Some(443), DeviceType::Phone);
    peers::save(&db, &[peer], SaveMode::Insert).unwrap();
    assert!(changes.has_changed().unwrap());
    changes.borrow_and_update();
    db.clone()
        .with_conn(|connection| {
            connection.execute("UPDATE peers SET port=5443 WHERE id='remote'", [])
        })
        .unwrap();
    assert!(changes.has_changed().unwrap());
    changes.borrow_and_update();
    db.with_conn(|connection| {
        connection.execute("UPDATE peers SET name='changed' WHERE id='remote'", [])
    })
    .unwrap();
    db.with_conn(|connection| connection.execute("DELETE FROM peers", []))
        .unwrap();
    assert!(changes.has_changed().unwrap());
    assert!(peers::get(&db, "remote").unwrap().is_none());
    changes.borrow_and_update();
    db.with_conn(|connection| connection.execute("DELETE FROM chats", []))
        .unwrap();
    assert!(!changes.has_changed().unwrap());
}

#[test]
fn transaction_invalidation_is_read_as_final_state_after_commit_or_rollback() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let peer = DPeer::new("remote", "remote", "", Some(443), DeviceType::Phone);
    peers::save(&db, &[peer], SaveMode::Insert).unwrap();
    let mut changes = db.peer_changes();
    db.with_conn(|connection| {
        let transaction = connection.unchecked_transaction().unwrap();
        transaction
            .execute("UPDATE peers SET port=5443 WHERE id='remote'", [])
            .unwrap();
        transaction
            .execute("UPDATE peers SET port=6443 WHERE id='remote'", [])
            .unwrap();
        transaction.commit().unwrap();
    });
    assert!(changes.has_changed().unwrap());
    changes.borrow_and_update();
    assert_eq!(peers::get(&db, "remote").unwrap().unwrap().port, Some(6443));
    db.with_conn(|connection| {
        let transaction = connection.unchecked_transaction().unwrap();
        transaction
            .execute("UPDATE peers SET port=7443 WHERE id='remote'", [])
            .unwrap();
        transaction.rollback().unwrap();
    });
    assert!(changes.has_changed().unwrap());
    assert_eq!(peers::get(&db, "remote").unwrap().unwrap().port, Some(6443));
}
