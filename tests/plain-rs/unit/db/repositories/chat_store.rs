use crate::chat::enums::{ChatStatus, DeviceType, PeerStatus};
use crate::db::chat_store::SaveMode;
use crate::db::{DChannel, DChat, DNearbyDeviceCache, DPeer, Db, chat_store};
use chat_store::{channels, messages, nearby, peers};
use serde_json::json;

#[test]
fn checked_chat_storage_is_atomic_literal_and_preserves_unrelated_conversations_and_login_tokens() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let mut peer = DPeer::new("peer", "name", "127.0.0.1", Some(443), DeviceType::Phone);
    peer.token = "session".into();
    peers::save(&db, std::slice::from_ref(&peer), SaveMode::Insert).unwrap();
    peer.name = "renamed".into();
    peer.token.clear();
    peers::save(&db, std::slice::from_ref(&peer), SaveMode::Update).unwrap();
    assert_eq!(peers::get(&db, "peer").unwrap().unwrap().token, "session");
    let before = peers::get(&db, "peer").unwrap().unwrap();
    let mut after = before.clone();
    after.ip = "::1".into();
    db.with_conn(|c| c.execute("UPDATE peers SET key='new-key' WHERE id='peer'", []))
        .unwrap();
    assert_eq!(
        peers::patch(&db, &before, &after).unwrap().unwrap().key,
        "new-key"
    );
    let missing = DPeer::new("missing", "", "", None, DeviceType::Unknown);
    peer.name = "must rollback".into();
    assert!(peers::save(&db, &[peer.clone(), missing], SaveMode::Update).is_err());
    assert_eq!(peers::get(&db, "peer").unwrap().unwrap().name, "renamed");
    let mut channel = DChannel::new("group", "me");
    channel.id = "channel".into();
    channel.version = 5_000_000_001;
    channel.members = r#"[{"peerId":"peer","status":"JOINED"}]"#.into();
    channels::save(&db, &[channel.clone()], SaveMode::Insert).unwrap();
    assert_eq!(
        channels::get(&db, "channel").unwrap().unwrap().version,
        5_000_000_001
    );
    let mut newer = channel.clone();
    newer.version += 1;
    channels::patch(&db, &channel, &newer).unwrap();
    assert!(channels::patch(&db, &channel, &channel).is_err());

    let mut one = DChat::new(
        "me",
        "peer",
        "",
        r#"{"type":"TEXT","value":{"text":"literal %_\\' 中文"}}"#,
    );
    one.id = "one".into();
    one.created_at = "2026-10-04T00:00:00.000Z".into();
    let mut two = DChat::new(
        "peer",
        "me",
        "",
        r#"{"type":"TEXT","value":{"text":"second"}}"#,
    );
    two.id = "two".into();
    two.created_at = "2026-10-04T00:00:01.000Z".into();
    let mut group = DChat::new(
        "peer",
        "me",
        "channel",
        r#"{"type":"TEXT","value":{"text":"group"}}"#,
    );
    group.id = "group".into();
    messages::save(&db, &[one.clone(), two.clone(), group], SaveMode::Insert).unwrap();
    let mut filter = messages::Filter {
        peer: Some("peer".into()),
        channel: None,
        text: String::new(),
        offset: 0,
        limit: Some(1),
        descending: true,
        latest: false,
        count_only: false,
    };
    let rows: Vec<DChat> = serde_json::from_value(messages::list(&db, &filter).unwrap()).unwrap();
    assert_eq!(rows[0].id, "two");
    filter.limit = None;
    filter.text = "%_".into();
    filter.count_only = true;
    assert_eq!(messages::list(&db, &filter).unwrap(), json!(1));
    filter.text = "%' OR 1=1 --".into();
    assert_eq!(messages::list(&db, &filter).unwrap(), json!(0));
    assert_eq!(messages::ids(&db, "peer:peer").unwrap().len(), 2);
    assert_eq!(
        messages::ids(&db, "channel:channel").unwrap(),
        vec!["group"]
    );
    assert!(messages::ids(&db, "text:unknown").unwrap().is_empty());
    assert_eq!(
        messages::ids(&db, r"ids:=one\ peer:other").unwrap(),
        vec!["one peer:other"]
    );
    assert_eq!(messages::ids(&db, r#"ids:" one ""#).unwrap(), vec![" one "]);
    assert!(messages::ids(&db, r#"ids:" ,  ,""#).unwrap().is_empty());
    assert_eq!(messages::delete(&db, &[]).unwrap(), 0);
    messages::status(&db, "one", ChatStatus::Partial, Some(r#"{"results":[]}"#)).unwrap();
    assert_eq!(
        messages::get(&db, "one").unwrap().unwrap().status,
        ChatStatus::Partial
    );
    assert!(messages::content(&db, "one", "malformed").is_err());
    assert_eq!(
        messages::get(&db, "one").unwrap().unwrap().content,
        one.content
    );
    let mut bad = two.clone();
    bad.id = "bad".into();
    bad.content = "bad".into();
    assert!(
        messages::save(
            &db,
            &[
                DChat {
                    id: "not-inserted".into(),
                    ..one
                },
                bad
            ],
            SaveMode::Insert
        )
        .is_err()
    );
    assert!(messages::get(&db, "not-inserted").unwrap().is_none());
    db.with_conn(|c| c.execute_batch("DROP TABLE peers;"))
        .unwrap();
    assert!(peers::all(&db).is_err());
}
#[test]
fn nearby_cache_keeps_newest_facts_and_all_records_survive_reopening() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("db");
    let db = Db::open(&path).unwrap();
    let mut item = DNearbyDeviceCache {
        id: "nearby".into(),
        name: "new".into(),
        ips: vec!["::1".into(), "127.0.0.1".into()],
        port: 443,
        device_type: "PHONE".into(),
        version: "1".into(),
        platform: "Android".into(),
        last_seen_at: "2026-10-04T00:00:02.000Z".into(),
    };
    nearby::save(&db, &item).unwrap();
    item.name = "stale".into();
    item.last_seen_at = "2026-10-04T00:00:01.000Z".into();
    nearby::save(&db, &item).unwrap();
    drop(db);
    let db = Db::open(&path).unwrap();
    let rows = nearby::all(&db).unwrap();
    assert_eq!(rows[0].name, "new");
    assert_eq!(rows[0].ips.len(), 2);
    assert!(!nearby::touch(&db, "absent").unwrap());
    assert!(nearby::touch(&db, "nearby").unwrap());
    assert!(nearby::delete(&db, "nearby").unwrap());
    assert!(nearby::all(&db).unwrap().is_empty());
}

#[test]
fn discovery_refreshes_only_paired_peers_and_preserves_identity_and_unrelated_rows() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let mut paired = DPeer::new("paired", "old", "192.0.2.1", Some(2443), DeviceType::Phone);
    paired.status = PeerStatus::Paired;
    paired.key = "paired-key".into();
    paired.public_key = "public-key".into();
    let unpaired = DPeer::new(
        "unpaired",
        "other",
        "192.0.2.2",
        Some(2443),
        DeviceType::Computer,
    );
    peers::save(&db, &[paired.clone(), unpaired.clone()], SaveMode::Insert).unwrap();
    let ips = vec!["192.0.2.9".into(), "2001:db8::9".into()];
    let updated = peers::discovered(&db, "paired", &ips, 8443, "renamed", DeviceType::Tablet)
        .unwrap()
        .unwrap();
    assert_eq!(updated.ip, "192.0.2.9,2001:db8::9");
    assert_eq!(updated.port, Some(8443));
    assert_eq!(updated.name, "renamed");
    assert_eq!(updated.device_type, DeviceType::Tablet);
    assert_eq!(updated.key, paired.key);
    assert_eq!(updated.public_key, paired.public_key);
    db.with_conn(|connection| connection.execute_batch("CREATE TRIGGER reject_repeat_discovery BEFORE UPDATE ON peers BEGIN SELECT RAISE(ABORT,'unexpected repeated update'); END;")).unwrap();
    assert!(
        peers::discovered(&db, "paired", &ips, 8443, "renamed", DeviceType::Tablet)
            .unwrap()
            .is_none()
    );
    assert!(
        peers::discovered(
            &db,
            "unpaired",
            &ips,
            8443,
            "unexpected",
            DeviceType::Tablet
        )
        .unwrap()
        .is_none()
    );
    let unchanged = peers::get(&db, "unpaired").unwrap().unwrap();
    assert_eq!(unchanged.ip, unpaired.ip);
    assert_eq!(unchanged.port, unpaired.port);
    assert_eq!(unchanged.name, unpaired.name);
    assert!(
        peers::discovered(&db, "unknown", &ips, 8443, "unknown", DeviceType::Phone)
            .unwrap()
            .is_none()
    );
}
