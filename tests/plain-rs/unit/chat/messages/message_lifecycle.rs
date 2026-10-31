use super::*;
use crate::chat::attachments::app_file_store;
use crate::chat::attachments::app_file_store::chat_deletion;
use crate::chat::attachments::app_file_store::chat_deletion::Selection;
use serde_json::{Value, json};
const TEXT: &str = r#"{"type":"TEXT","value":{"text":"fixture ' 中文"}}"#;
fn db() -> Db {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    db.upsert_peer(&DPeer::new(
        "peer",
        "Fixture",
        "",
        Some(1),
        crate::chat::enums::DeviceType::Phone,
    ));
    db
}
#[test]
fn creation_and_sql_failure_return_actual_committed_state() {
    let db = db();
    assert_eq!(
        create(&db, "local", "", TEXT).unwrap().status,
        ChatStatus::Sent
    );
    assert_eq!(
        create(&db, "peer", "", TEXT).unwrap().status,
        ChatStatus::Pending
    );
    assert!(create(&db, "peer", "group", TEXT).is_err());
    assert!(create(&db, "local", "", "{}").is_err());
    db.with_conn(|c|c.execute_batch("CREATE TRIGGER fail_chat BEFORE INSERT ON chats BEGIN SELECT RAISE(ABORT,'fixture failure'); END;")).unwrap();
    assert!(create(&db, "local", "", TEXT).is_err());
}
#[test]
fn reception_is_atomic_and_receipt_survives_restart_and_message_deletion() {
    let path = std::env::temp_dir().join(format!(
        "chat-receipt-{}-{}.db",
        std::process::id(),
        crate::db::short_id()
    ));
    let db = Db::open(&path).unwrap();
    db.upsert_peer(&DPeer::new(
        "peer",
        "Fixture",
        "",
        Some(1),
        crate::chat::enums::DeviceType::Phone,
    ));
    let signature = crate::base64_encode(&[7; 64]);
    let timestamp = crate::peers::pairing::now_ms();
    db.with_conn(|c|c.execute_batch("CREATE TRIGGER fail_chat BEFORE INSERT ON chats BEGIN SELECT RAISE(ABORT,'fixture failure'); END;")).unwrap();
    assert!(receive(&db, "peer", "", TEXT, &signature, timestamp).is_err());
    db.with_conn(|c| c.execute_batch("DROP TRIGGER fail_chat;"))
        .unwrap();
    let first = receive(&db, "peer", "", TEXT, &signature, timestamp)
        .unwrap()
        .unwrap();
    assert!(
        receive(&db, "peer", "", TEXT, &signature, timestamp)
            .unwrap()
            .is_none()
    );
    messages::delete(&db, &[first.chat.id]).unwrap();
    drop(db);
    let db = Db::open(&path).unwrap();
    assert!(
        receive(&db, "peer", "", TEXT, &signature, timestamp)
            .unwrap()
            .is_none()
    );
    assert!(receive(&db, "unknown", "", TEXT, &signature, timestamp).is_err());
    assert!(receive(&db, "peer", "", TEXT, &signature, i64::MIN).is_err());
    crate::db::chat_store::peers::delete(&db, &["peer".into()]).unwrap();
    assert_eq!(
        db.with_conn(
            |c| c.query_row("SELECT count(*) FROM chat_receipts", [], |r| r
                .get::<_, i64>(0))
        )
        .unwrap(),
        0
    );
    drop(db);
    std::fs::remove_file(path).unwrap();
}
#[test]
fn retry_merges_current_recipient_results_and_protects_content_and_creation() {
    let db = db();
    let chat = create(&db, "peer", "", TEXT).unwrap();
    let result = |id: &str, error: Option<&str>| ChannelDeliveryResult {
        peer_id: id.into(),
        peer_name: id.into(),
        error: error.map(str::to_owned),
    };
    let first = delivery(
        &db,
        &chat.id,
        Some(vec![result("one", None), result("two", Some("offline"))]),
        false,
    )
    .unwrap()
    .unwrap();
    assert_eq!(first.status, ChatStatus::Partial);
    let retried = delivery(&db, &chat.id, Some(vec![result("two", None)]), true)
        .unwrap()
        .unwrap();
    assert_eq!(retried.status, ChatStatus::Sent);
    assert_eq!(retried.content, chat.content);
    assert_eq!(retried.created_at, chat.created_at);
    let data: StatusData = serde_json::from_str(retried.status_data.as_deref().unwrap()).unwrap();
    assert_eq!(data.results.unwrap().len(), 2);
    assert!(
        delivery(
            &db,
            &chat.id,
            Some(vec![result("two", None), result("two", None)]),
            true
        )
        .is_err()
    );
    messages::delete(&db, &[chat.id.clone()]).unwrap();
    assert!(
        delivery(&db, &chat.id, Some(vec![]), false)
            .unwrap()
            .is_none()
    );
}

#[test]
fn forwarding_creates_one_atomic_batch_from_the_stored_message() {
    let db = db();
    let source = create(&db, "local", "", TEXT).unwrap();
    let targets = vec![
        ("local".into(), String::new()),
        ("peer".into(), String::new()),
    ];
    db.with_conn(|connection| connection.execute_batch("CREATE TRIGGER reject_forward BEFORE INSERT ON chats WHEN NEW.to_id='peer' BEGIN SELECT RAISE(ABORT,'fixture failure'); END;")).unwrap();
    assert!(forward_many(&db, &source.id, &targets).is_err());
    assert_eq!(db.get_all_chats().len(), 1);
    db.with_conn(|connection| connection.execute_batch("DROP TRIGGER reject_forward;"))
        .unwrap();
    let rows = forward_many(&db, &source.id, &targets).unwrap();
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .all(|row| row.content == TEXT && row.from_id == "me")
    );
    assert_eq!(rows[0].status, ChatStatus::Sent);
    assert_eq!(rows[1].status, ChatStatus::Pending);
    assert_ne!(rows[0].id, rows[1].id);
    assert_ne!(rows[0].id, source.id);
    assert!(forward_many(&db, "missing", &targets).is_err());
    assert!(forward_many(&db, "missing", &[]).unwrap().is_empty());
    assert_eq!(db.get_all_chats().len(), 3);
}

#[test]
fn channel_reception_requires_current_joined_membership_before_consuming_receipt() {
    use crate::chat::channels::messages::ChannelMember;
    use crate::db::chat_store::{SaveMode, channels};
    let db = db();
    let mut channel = crate::chat::channels::state::create(&db, "owner", "Fixture").unwrap();
    let signature = crate::base64_encode(&[19; 64]);
    let timestamp = crate::peers::pairing::now_ms();
    for members in [
        serde_json::to_string(&[ChannelMember::new("owner")]).unwrap(),
        serde_json::to_string(&[ChannelMember::new("owner"), ChannelMember::pending("peer")])
            .unwrap(),
        "invalid roster".to_owned(),
    ] {
        db.with_conn(|connection| {
            connection.execute(
                "UPDATE chat_channels SET members=?1 WHERE id=?2",
                rusqlite::params![members, channel.id],
            )
        })
        .unwrap();
        assert!(receive(&db, "peer", &channel.id, TEXT, &signature, timestamp).is_err());
        db.with_conn(|connection| -> anyhow::Result<()> {
            assert_eq!(
                connection
                    .query_row("SELECT COUNT(*) FROM chats", [], |row| row.get::<_, i64>(0))?,
                0
            );
            assert_eq!(
                connection.query_row("SELECT COUNT(*) FROM chat_receipts", [], |row| row
                    .get::<_, i64>(0))?,
                0
            );
            Ok(())
        })
        .unwrap();
    }
    channel.members =
        serde_json::to_string(&[ChannelMember::new("owner"), ChannelMember::new("peer")]).unwrap();
    channels::save(&db, std::slice::from_ref(&channel), SaveMode::Update).unwrap();
    let received = receive(&db, "peer", &channel.id, TEXT, &signature, timestamp)
        .unwrap()
        .unwrap();
    assert_eq!(
        received.chat.channel_id.as_deref(),
        Some(channel.id.as_str())
    );
    assert!(
        receive(&db, "peer", &channel.id, TEXT, &signature, timestamp)
            .unwrap()
            .is_none()
    );
    channel.members = serde_json::to_string(&[ChannelMember::new("owner")]).unwrap();
    channels::save(&db, std::slice::from_ref(&channel), SaveMode::Update).unwrap();
    assert!(
        receive(
            &db,
            "peer",
            &channel.id,
            TEXT,
            &crate::base64_encode(&[20; 64]),
            timestamp
        )
        .is_err()
    );
    db.with_conn(|connection| -> anyhow::Result<()> {
        assert_eq!(
            connection.query_row("SELECT COUNT(*) FROM chats", [], |row| row.get::<_, i64>(0))?,
            1
        );
        assert_eq!(
            connection.query_row("SELECT COUNT(*) FROM chat_receipts", [], |row| row
                .get::<_, i64>(0))?,
            1
        );
        Ok(())
    })
    .unwrap();
}

fn attachment_fixture() -> (tempfile::TempDir, Db, app_file_store::ImportResult) {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("plain.db")).unwrap();
    let file =
        app_file_store::import_bytes(&db, dir.path(), b"synthetic owned attachment", "text/plain")
            .unwrap();
    (dir, db, file)
}
fn attachment_item(id: &str, file: &app_file_store::ImportResult) -> Value {
    json!({"id":id,"uri":format!("fid:{}",file.fid_suffix),"fileName":"fixture.txt","size":26,"loading":false})
}
fn attachment_refs(db: &Db, id: &str) -> i64 {
    db.with_conn(|c| {
        c.query_row("SELECT ref_count FROM app_files WHERE id=?1", [id], |r| {
            r.get(0)
        })
    })
    .unwrap()
}
fn create_attachments(
    db: &Db,
    destination: &str,
    items: Vec<Value>,
    images: bool,
) -> Result<DChat> {
    create(
        db,
        destination.strip_prefix("peer:").unwrap_or(destination),
        "",
        &json!({"type":if images {"IMAGES"} else {"FILES"},"value":{"items":items}}).to_string(),
    )
}
#[test]
fn one_import_shared_across_targets_keeps_physical_content_until_last_owner() {
    let (dir, db, file) = attachment_fixture();
    let first = create_attachments(
        &db,
        "peer:local",
        vec![attachment_item("file", &file)],
        false,
    )
    .unwrap();
    assert_eq!(attachment_refs(&db, &file.id), 1);
    let second = create_attachments(
        &db,
        "peer:other",
        vec![attachment_item("file", &file)],
        false,
    )
    .unwrap();
    assert_eq!(attachment_refs(&db, &file.id), 2);
    chat_deletion::delete(&db, dir.path(), Selection::Ids(&[first.id])).unwrap();
    assert_eq!(attachment_refs(&db, &file.id), 1);
    assert!(file.real_path.exists());
    chat_deletion::delete(&db, dir.path(), Selection::Ids(&[second.id])).unwrap();
    assert!(!file.real_path.exists());
}
#[test]
fn a_fresh_deduplicated_import_is_consumed_once_without_double_increment() {
    let (dir, db, file) = attachment_fixture();
    create_attachments(&db, "local", vec![attachment_item("first", &file)], true).unwrap();
    let imported =
        app_file_store::import_bytes(&db, dir.path(), b"synthetic owned attachment", "text/plain")
            .unwrap();
    assert_eq!(attachment_refs(&db, &file.id), 2);
    create_attachments(
        &db,
        "peer:other",
        vec![attachment_item("second", &imported)],
        true,
    )
    .unwrap();
    assert_eq!(attachment_refs(&db, &file.id), 2);
}
#[test]
fn failed_attachment_batch_does_not_claim_imports_or_leave_partial_messages() {
    let (_dir, db, file) = attachment_fixture();
    let content =
        json!({"type":"FILES","value":{"items":[attachment_item("file", &file)]}}).to_string();
    db.with_conn(|connection| connection.execute_batch("CREATE TRIGGER reject_second BEFORE INSERT ON chats WHEN NEW.to_id='other' BEGIN SELECT RAISE(FAIL,'synthetic refusal'); END")).unwrap();
    assert!(
        create_many(
            &db,
            &[("local".into(), "".into()), ("other".into(), "".into())],
            &content
        )
        .is_err()
    );
    assert!(db.get_all_chats().is_empty());
    assert_eq!(attachment_refs(&db, &file.id), 1);
    assert!(file.real_path.exists());
    assert!(create(&db, "local", "", &json!({"type":"FILES","value":{"items":[json!({"id":"missing","uri":"fid:missing.txt","size":1})]}}).to_string()).is_err());
    assert!(db.get_all_chats().is_empty());
    assert_eq!(attachment_refs(&db, &file.id), 1);
}

fn assert_invalid_attachment(content: Value, db: &Db, file: &app_file_store::ImportResult) {
    let content = content.to_string();
    let signature = crate::base64_encode(&[7; 64]);
    db.upsert_peer(&DPeer::new(
        "peer",
        "Fixture",
        "",
        Some(1),
        crate::chat::enums::DeviceType::Phone,
    ));
    assert!(
        create_many(
            db,
            &[("local".into(), "".into()), ("peer".into(), "".into())],
            &content
        )
        .is_err(),
        "accepted outgoing {content}"
    );
    assert!(
        receive(
            db,
            "peer",
            "",
            &content,
            &signature,
            crate::peers::pairing::now_ms()
        )
        .is_err(),
        "accepted incoming {content}"
    );
    assert!(db.get_all_chats().is_empty());
    assert_eq!(
        db.with_conn(
            |c| c.query_row("SELECT count(*) FROM chat_receipts", [], |r| r
                .get::<_, i64>(0))
        )
        .unwrap(),
        0
    );
    assert_eq!(attachment_refs(db, &file.id), 1);
    assert!(file.real_path.exists());
}

#[test]
fn malformed_attachment_lists_never_write_messages_or_receipts() {
    let (_dir, db, file) = attachment_fixture();
    for kind in ["FILES", "IMAGES"] {
        for items in [
            json!({}),
            json!(null),
            json!("items"),
            json!([null]),
            json!([42]),
        ] {
            assert_invalid_attachment(json!({"type":kind,"value":{"items":items}}), &db, &file);
        }
    }
}

#[test]
fn invalid_attachment_fields_never_claim_files_or_consume_receipts() {
    let (_dir, db, file) = attachment_fixture();
    for kind in ["FILES", "IMAGES"] {
        for (field, invalid) in [
            ("size", json!(-1)),
            ("size", json!(1.5)),
            ("size", json!("4")),
            ("size", json!(null)),
            ("size", json!(u64::MAX)),
            ("durationMs", json!(-1)),
            ("durationMs", json!(0.5)),
            ("duration", json!(-1)),
            ("duration", json!(i64::MAX)),
            ("legacyDurationSec", json!(-1)),
            ("legacyDurationSec", json!(i64::MAX)),
            ("width", json!(-1)),
            ("height", json!(i64::MAX)),
            ("uri", json!(null)),
            ("uri", json!(42)),
            ("fileName", json!(42)),
            ("summary", json!(null)),
            ("id", json!(false)),
        ] {
            let mut item = attachment_item("file", &file);
            item[field] = invalid;
            assert_invalid_attachment(json!({"type":kind,"value":{"items":[item]}}), &db, &file);
        }
    }
}

#[test]
fn valid_attachment_defaults_legacy_duration_and_empty_lists_remain_supported() {
    let (_dir, db, file) = attachment_fixture();
    for kind in ["FILES", "IMAGES"] {
        for item in [
            json!({"uri":format!("fid:{}", file.fid_suffix),"size":0}),
            json!({"uri":format!("fid:{}", file.fid_suffix),"size":26,"duration":2}),
            json!({"uri":format!("fid:{}", file.fid_suffix),"size":26,"legacyDurationSec":2}),
            json!({"uri":format!("fid:{}", file.fid_suffix),"size":26,"durationMs":null,"duration":null}),
            json!({"uri":format!("fid:{}", file.fid_suffix),"size":26,"durationMs":3000,"width":320,"height":240,"extra":true}),
        ] {
            let content = json!({"type":kind,"value":{"items":[item]}}).to_string();
            assert_eq!(create(&db, "local", "", &content).unwrap().content, content);
        }
        let content = json!({"type":kind,"value":{"items":[]}}).to_string();
        assert_eq!(create(&db, "local", "", &content).unwrap().content, content);
    }
}
