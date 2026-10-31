use super::*;
fn facts() -> Facts {
    Facts {
        id: "peer".into(),
        name: "name ' 中文".into(),
        ips: vec!["127.0.0.1".into(), "".into(), "127.0.0.1".into()],
        port: 2443,
        device_type: DeviceType::Phone,
        key: crate::base64_encode(&[1; 32]),
        public_key: crate::base64_encode(&[2; 32]),
    }
}
#[test]
fn repairing_preserves_creation_and_independent_token_and_rolls_back_errors() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let first = save(&db, facts()).unwrap();
    assert_eq!(first.ip, "127.0.0.1");
    db.with_conn(|c| {
        c.execute(
            "UPDATE peers SET token='keep',created_at='2000-01-01T00:00:00.000Z' WHERE id='peer'",
            [],
        )
    })
    .unwrap();
    let mut next = facts();
    next.key = crate::base64_encode(&[3; 32]);
    let updated = save(&db, next).unwrap();
    assert_eq!(updated.token, "keep");
    assert_eq!(updated.created_at, "2000-01-01T00:00:00.000Z");
    assert_eq!(updated.key, crate::base64_encode(&[3; 32]));
    db.with_conn(|c|c.execute_batch("CREATE TRIGGER fail_pair BEFORE UPDATE ON peers BEGIN SELECT RAISE(ABORT,'fixture failure'); END;")).unwrap();
    assert!(save(&db, facts()).is_err());
    assert_eq!(db.get_peer_by_id("peer").unwrap().key, updated.key);
    let mut invalid = facts();
    invalid.key = "bad".into();
    assert!(save(&db, invalid).is_err());
}

#[test]
fn a_known_signing_identity_cannot_be_replaced_by_repairing() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let previous = save(&db, facts()).unwrap();
    let mut replacement = facts();
    replacement.public_key = crate::base64_encode(&[9; 32]);
    replacement.key = crate::base64_encode(&[8; 32]);
    assert!(save(&db, replacement).is_err());
    let current = db.get_peer_by_id("peer").unwrap();
    assert_eq!(current.public_key, previous.public_key);
    assert_eq!(current.key, previous.key);
}

#[test]
fn unpair_removes_management_access_before_chat_only_repairing_and_preserves_history() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let original = save(&db, facts()).unwrap();
    db.with_conn(|connection| {
        connection.execute(
            "UPDATE peers SET token='management-grant' WHERE id='peer'",
            [],
        )
    })
    .unwrap();
    let chat = crate::db::DChat::new(
        "me",
        "peer",
        "",
        r#"{"type":"TEXT","value":{"text":"saved message"}}"#,
    );
    crate::db::chat_store::messages::save(
        &db,
        std::slice::from_ref(&chat),
        crate::db::chat_store::SaveMode::Insert,
    )
    .unwrap();
    assert!(crate::db::chat_store::peers::unpair(&db, "peer").unwrap());
    let unpaired = crate::db::chat_store::peers::get(&db, "peer")
        .unwrap()
        .unwrap();
    assert_eq!(unpaired.status, PeerStatus::Unpaired);
    assert!(unpaired.token.is_empty());
    assert_eq!(unpaired.public_key, original.public_key);
    let repaired = save(&db, facts()).unwrap();
    assert_eq!(repaired.status, PeerStatus::Paired);
    assert!(repaired.token.is_empty());
    assert_eq!(repaired.created_at, original.created_at);
    assert_eq!(
        crate::db::chat_store::messages::get(&db, &chat.id)
            .unwrap()
            .unwrap()
            .content,
        chat.content
    );
}
