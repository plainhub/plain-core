use super::*;
use crate::chat::enums::ChannelStatus;
use crate::chat::enums::DeviceType;
use crate::chat::enums::PeerStatus;
use crate::db::DChannel;
use crate::db::DPeer;
use crate::db::Db;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_tmp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("plain-rs-chat-cacher-{label}-{pid}-{nanos}"))
}

fn seed_chat(db: &Db, id: &str, from_id: &str, to_id: &str, channel_id: &str) {
    let mut chat = DChat::new(from_id, to_id, channel_id, "{}");
    chat.id = id.to_string();
    db.insert_chat(&chat);
}

fn seed_peer(db: &Db, id: &str) {
    let now = crate::db::now_iso();
    let peer = DPeer {
        id: id.to_string(),
        name: id.to_string(),
        ip: String::new(),
        key: String::new(),
        public_key: String::new(),
        status: PeerStatus::Paired,
        port: None,
        device_type: DeviceType::Unknown,
        token: String::new(),
        created_at: now.clone(),
        updated_at: now,
    };
    db.upsert_peer(&peer);
}

fn seed_channel(db: &Db, id: &str) {
    let mut ch = DChannel::new(id, "me");
    ch.id = id.to_string();
    ch.status = ChannelStatus::Joined;
    db.insert_channel(&ch);
}

#[test]
fn load_caches_local_chat() {
    let db = Db::open(&unique_tmp_dir("local").join("plain.db")).expect("open db");
    seed_chat(&db, "a", "me", "local", "");

    let cacher = ChatCacher::new();
    cacher.load(&db);

    assert!(cacher.get_latest_chat("local").is_some());
}

#[test]
fn load_caches_peer_chat() {
    let db = Db::open(&unique_tmp_dir("peer").join("plain.db")).expect("open db");
    seed_peer(&db, "peer1");
    seed_chat(&db, "a", "me", "peer1", "");

    let cacher = ChatCacher::new();
    cacher.load(&db);

    assert!(cacher.get_latest_chat("peer1").is_some());
}

#[test]
fn load_caches_channel_chat() {
    let db = Db::open(&unique_tmp_dir("chan").join("plain.db")).expect("open db");
    seed_channel(&db, "ch1");
    seed_chat(&db, "a", "me", "", "ch1");

    let cacher = ChatCacher::new();
    cacher.load(&db);

    assert!(cacher.get_latest_chat("ch1").is_some());
}

#[test]
fn load_skips_chats_for_unknown_peer() {
    let db = Db::open(&unique_tmp_dir("unknown").join("plain.db")).expect("open db");
    seed_chat(&db, "a", "me", "ghost", "");

    let cacher = ChatCacher::new();
    cacher.load(&db);

    assert!(cacher.get_latest_chat("ghost").is_none());
}

#[test]
fn load_keeps_most_recent_for_same_conversation() {
    let db = Db::open(&unique_tmp_dir("latest").join("plain.db")).expect("open db");
    // now_iso() has seconds resolution, so manually set timestamps
    // to guarantee ordering.
    let mut old = DChat::new("me", "local", "", "{}");
    old.id = "old".to_string();
    old.created_at = "2026-01-01T00:00:01.000Z".to_string();
    old.updated_at = "2026-01-01T00:00:01.000Z".to_string();
    db.insert_chat(&old);

    let mut new = DChat::new("me", "local", "", "{}");
    new.id = "new".to_string();
    new.created_at = "2026-01-01T00:00:02.000Z".to_string();
    new.updated_at = "2026-01-01T00:00:02.000Z".to_string();
    db.insert_chat(&new);

    let cacher = ChatCacher::new();
    cacher.load(&db);

    let latest = cacher.get_latest_chat("local").unwrap();
    assert_eq!(latest.id, "new");
}

#[test]
fn conversations_unify_both_directions_and_edits_do_not_promote_older_messages() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    seed_peer(&db, "peer1");
    seed_channel(&db, "group1");
    for (id, from, to, channel, created, updated) in [
        ("old", "me", "peer1", "", 1, 9),
        ("reply", "peer1", "me", "", 2, 2),
        ("local", "me", "local", "", 3, 3),
        ("group", "peer1", "", "group1", 4, 4),
        ("unknown", "me", "ghost", "", 5, 5),
        ("unknown-group", "peer1", "me", "ghost-group", 6, 6),
    ] {
        let mut chat = DChat::new(from, to, channel, "{}");
        chat.id = id.into();
        chat.created_at = format!("2026-01-01T00:00:{created:02}.000Z");
        chat.updated_at = format!("2026-01-01T00:00:{updated:02}.000Z");
        db.insert_chat(&chat).unwrap();
    }
    let rows = ChatCacher::conversations(&db).unwrap();
    assert_eq!(
        rows.iter()
            .map(|(id, chat)| (id.as_str(), chat.id.as_str()))
            .collect::<Vec<_>>(),
        [("group1", "group"), ("local", "local"), ("peer1", "reply")]
    );
    assert_eq!(ChatCacher::snapshot(&db).unwrap().len(), 3);
}

#[test]
fn equal_creation_times_choose_one_stable_message_and_an_empty_store_has_no_conversations() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    assert!(ChatCacher::conversations(&db).unwrap().is_empty());
    for id in ["a", "b"] {
        let mut chat = DChat::new("me", "local", "", "{}");
        chat.id = id.into();
        chat.created_at = "2026-01-01T00:00:00.000Z".into();
        db.insert_chat(&chat).unwrap();
    }
    let rows = ChatCacher::conversations(&db).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].1.id, "b");
}
