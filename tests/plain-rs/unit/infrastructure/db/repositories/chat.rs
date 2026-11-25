use super::*;
use crate::infrastructure::db::chat_tests::unique_tmp_dir;

fn seed_chat(db: &Db, id: &str, from_id: &str, to_id: &str, channel_id: &str) {
    let mut chat = DChat::new(from_id, to_id, channel_id, "{}");
    chat.id = id.to_string();
    db.insert_chat(&chat);
}

#[test]
fn get_chats_by_peer_returns_both_directions() {
    let db = Db::open(&unique_tmp_dir("peer-both").join("plain.db")).expect("open db");
    seed_chat(&db, "a", "me", "peer1", "");
    seed_chat(&db, "b", "peer1", "me", "");
    seed_chat(&db, "c", "me", "peer2", "");
    seed_chat(&db, "d", "me", "peer1", "ch1");

    let ids: Vec<String> = db
        .get_chats_by_peer("peer1")
        .into_iter()
        .map(|c| c.id)
        .collect();
    assert_eq!(ids, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn delete_chats_by_peer_preserves_channel_chats() {
    let db = Db::open(&unique_tmp_dir("peer-del").join("plain.db")).expect("open db");
    seed_chat(&db, "a", "me", "peer1", "");
    seed_chat(&db, "b", "peer1", "me", "");
    seed_chat(&db, "c", "me", "peer1", "ch1");

    db.delete_chats_by_peer("peer1");

    assert!(db.get_chat_by_id("a").is_none());
    assert!(db.get_chat_by_id("b").is_none());
    assert!(db.get_chat_by_id("c").is_some());
}

#[test]
fn delete_chats_by_ids_removes_only_listed_ids() {
    let db = Db::open(&unique_tmp_dir("ids-del").join("plain.db")).expect("open db");
    seed_chat(&db, "a", "me", "p", "");
    seed_chat(&db, "b", "me", "p", "");
    seed_chat(&db, "c", "me", "p", "");

    db.delete_chats_by_ids(&["a".to_string(), "c".to_string()]);

    assert!(db.get_chat_by_id("a").is_none());
    assert!(db.get_chat_by_id("b").is_some());
    assert!(db.get_chat_by_id("c").is_none());
}

#[test]
fn delete_chats_by_ids_noop_for_empty_list() {
    let db = Db::open(&unique_tmp_dir("ids-empty").join("plain.db")).expect("open db");
    seed_chat(&db, "a", "me", "p", "");
    db.delete_chats_by_ids(&[]);
    assert!(db.get_chat_by_id("a").is_some());
}

#[test]
fn get_chats_by_channel_returns_only_that_channel() {
    let db = Db::open(&unique_tmp_dir("chan-get").join("plain.db")).expect("open db");
    seed_chat(&db, "a", "me", "", "ch1");
    seed_chat(&db, "b", "me", "", "ch2");
    seed_chat(&db, "c", "me", "", "ch1");

    let ids: Vec<String> = db
        .get_chats_by_channel("ch1")
        .into_iter()
        .map(|c| c.id)
        .collect();
    assert_eq!(ids, vec!["a".to_string(), "c".to_string()]);
}
