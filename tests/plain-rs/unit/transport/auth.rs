use super::*;
use crate::base64_encode;
use crate::chat::enums::DeviceType;
use crate::chat::enums::PeerStatus;
use crate::db::DPeer;
use crate::db::Db;
use crate::ed25519_generate;
use crate::ed25519_sign;
use crate::xchacha_encrypt_raw;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_db() -> (Db, std::path::PathBuf) {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "plain-rs-chat-peerauth-{}-{seq}-{}.db",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    let db = Db::open(&path).unwrap();
    (db, path)
}

/// Seed a paired peer with the given shared key and return (peer_id,
/// key, kp_bytes).
fn seed_paired_peer(db: &Db) -> (String, [u8; 32], [u8; 64]) {
    let (kp, vk) = ed25519_generate();
    let key = [77u8; 32];
    let peer = DPeer {
        id: "peer-a".into(),
        name: "Phone".into(),
        ip: "203.0.113.4".into(),
        key: base64_encode(&key),
        public_key: base64_encode(&vk),
        status: PeerStatus::Paired,
        port: Some(2443),
        device_type: DeviceType::Phone,
        token: String::new(),
        created_at: "2026-01-01T00:00:00.000Z".into(),
        updated_at: "2026-01-01T00:00:00.000Z".into(),
    };
    db.upsert_peer(&peer);
    ("peer-a".into(), key, kp)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// Build an encrypted+signed request body exactly like the sender side
/// (`deliver_to_peer` wire format).
fn build_body(kp: &[u8], key: &[u8; 32], graphql_json: &str, ts: i64) -> Vec<u8> {
    let sig_data = signature_payload(ts, "", graphql_json);
    let signature = ed25519_sign(kp, sig_data.as_bytes());
    let payload = format!("{signature}|{ts}|{graphql_json}");
    xchacha_encrypt_raw(key, payload.as_bytes()).expect("encrypt")
}

#[test]
fn authenticate_roundtrip_paired_peer() {
    let (db, _path) = unique_db();
    let (peer_id, key, kp) = seed_paired_peer(&db);
    let graphql_json = r#"{"query":"mutation { createChatItem(content: \"x\") { id } }"}"#;
    let body = build_body(&kp, &key, graphql_json, now_ms());

    let authed = authenticate(&db, &peer_id, "", &body).expect("auth ok");
    assert_eq!(authed.peer.id, "peer-a");
    assert_eq!(authed.graphql_json, graphql_json);
    assert_eq!(authed.key, key.to_vec());
}

#[test]
fn authenticate_rejects_unknown_peer_and_unpaired() {
    let (db, _path) = unique_db();
    let (_, key, kp) = seed_paired_peer(&db);
    let body = build_body(&kp, &key, "{}", now_ms());

    assert!(matches!(
        authenticate(&db, "ghost", "", &body),
        Err(AuthError::UnknownPeer)
    ));

    // Demote to CHANNEL → direct message rejected, channel message OK.
    db.update_peer_status("peer-a", PeerStatus::Channel);
    assert!(matches!(
        authenticate(&db, "peer-a", "", &body),
        Err(AuthError::NotPaired)
    ));
    db.update_peer_status("peer-a", PeerStatus::Paired);
}

#[test]
fn authenticate_rejects_wrong_key_and_bad_signature() {
    let (db, _path) = unique_db();
    let (peer_id, key, kp) = seed_paired_peer(&db);

    // Encrypted with a different key → DecryptFailed.
    let wrong_key = [1u8; 32];
    let body = build_body(&kp, &wrong_key, "{}", now_ms());
    assert!(matches!(
        authenticate(&db, &peer_id, "", &body),
        Err(AuthError::DecryptFailed)
    ));

    // Signed by a different keypair → BadSignature.
    let (_other_kp, other_vk) = ed25519_generate();
    let mut peer = db.get_peer_by_id("peer-a").unwrap();
    peer.public_key = base64_encode(&other_vk);
    db.upsert_peer(&peer);
    let body = build_body(&kp, &key, "{}", now_ms());
    assert!(matches!(
        authenticate(&db, &peer_id, "", &body),
        Err(AuthError::BadSignature)
    ));
}

#[test]
fn authenticate_rejects_expired_timestamp_and_missing_fields() {
    let (db, _path) = unique_db();
    let (peer_id, key, kp) = seed_paired_peer(&db);

    // Six minutes old.
    let body = build_body(&kp, &key, "{}", now_ms() - 6 * 60 * 1000);
    assert!(matches!(
        authenticate(&db, &peer_id, "", &body),
        Err(AuthError::TimestampExpired)
    ));

    // Missing signature part.
    let payload = format!("|{}|{{}}", now_ms());
    let body = xchacha_encrypt_raw(&key, payload.as_bytes()).unwrap();
    assert!(matches!(
        authenticate(&db, &peer_id, "", &body),
        Err(AuthError::MissingFields)
    ));
}

#[test]
fn authenticate_channel_message_uses_channel_key_without_paired_check() {
    let (db, _path) = unique_db();
    let (_, peer_key, kp) = seed_paired_peer(&db);
    // Channel member (not paired) with a channel key in the cache.
    db.update_peer_status("peer-a", PeerStatus::Channel);
    let channel_key = [42u8; 32];
    let mut channel = crate::db::DChannel::new("channel", "owner");
    channel.id = "ch-1".into();
    channel.key = base64_encode(&channel_key);
    crate::db::chat_store::channels::save(&db, &[channel], crate::db::chat_store::SaveMode::Insert)
        .unwrap();
    let graphql_json =
        r#"{"query":"mutation { channelSystemMessage(type: INVITE, payload: \"{}\") }"}"#;
    let timestamp = now_ms();
    let signature = ed25519_sign(
        &kp,
        signature_payload(timestamp, "ch-1", graphql_json).as_bytes(),
    );
    let body = xchacha_encrypt_raw(
        &channel_key,
        format!("{signature}|{timestamp}|{graphql_json}").as_bytes(),
    )
    .unwrap();

    // With c-cid set, the channel key decrypts and no paired check fires.
    let authed = authenticate(&db, "peer-a", "ch-1", &body).expect("channel auth ok");
    assert_eq!(authed.graphql_json, graphql_json);
    assert_eq!(authed.key, channel_key.to_vec());

    // Without a cached channel key → NoChannelKey.
    crate::db::chat_store::channels::delete(&db, &["ch-1".into()]).unwrap();
    assert!(matches!(
        authenticate(&db, "peer-a", "ch-1", &body),
        Err(AuthError::NoChannelKey)
    ));
    // Sanity: the peer's own key would NOT decrypt this body.
    let wrong = build_body(&kp, &peer_key, graphql_json, now_ms());
    assert!(matches!(
        authenticate(&db, "peer-a", "", &wrong),
        Err(AuthError::NotPaired)
    ));
}

#[test]
fn extreme_timestamp_and_malformed_envelopes_do_not_panic_and_storage_failures_are_real() {
    let (db, _) = unique_db();
    let (id, key, kp) = seed_paired_peer(&db);
    for timestamp in [i64::MIN, i64::MAX] {
        assert!(matches!(
            authenticate(&db, &id, "", &build_body(&kp, &key, "{}", timestamp)),
            Err(AuthError::TimestampExpired)
        ));
    }
    let malformed = xchacha_encrypt_raw(&key, b"signature|not-a-number|{}").unwrap();
    assert!(matches!(
        authenticate(&db, &id, "", &malformed),
        Err(AuthError::MissingFields)
    ));
    db.with_conn(|c| c.execute_batch("DROP TABLE peers"))
        .unwrap();
    assert!(matches!(
        authenticate(&db, &id, "", &build_body(&kp, &key, "{}", now_ms())),
        Err(AuthError::Storage)
    ));
}

#[test]
fn signed_context_rejects_reencrypted_cross_channel_direct_and_legacy_frames() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let (peer, key, kp) = seed_paired_peer(&db);
    let mut channels = Vec::new();
    for (id, key) in [("first", [41; 32]), ("second", [42; 32])] {
        let mut channel = crate::db::DChannel::new(id, "owner");
        channel.id = id.into();
        channel.key = base64_encode(&key);
        channels.push(channel);
    }
    crate::db::chat_store::channels::save(&db, &channels, crate::db::chat_store::SaveMode::Insert)
        .unwrap();
    let content = r#"{"query":"mutation($content:String!){createChatItem(content:$content){id}}","variables":{"content":"{\"type\":\"TEXT\",\"value\":{\"text\":\"context\"}}"}}"#;
    let timestamp = now_ms();
    let signature = ed25519_sign(
        &kp,
        signature_payload(timestamp, "first", content).as_bytes(),
    );
    let envelope = format!("{signature}|{timestamp}|{content}");
    let first = xchacha_encrypt_raw(&[41; 32], envelope.as_bytes()).unwrap();
    assert!(authenticate(&db, &peer, "first", &first).is_ok());
    for (channel, key) in [("second", vec![42; 32]), ("", key.to_vec())] {
        let body = xchacha_encrypt_raw(&key, envelope.as_bytes()).unwrap();
        assert!(matches!(
            authenticate(&db, &peer, channel, &body),
            Err(AuthError::BadSignature)
        ));
    }
    let legacy = ed25519_sign(&kp, format!("{timestamp}{content}").as_bytes());
    let body = xchacha_encrypt_raw(
        &[41; 32],
        format!("{legacy}|{timestamp}|{content}").as_bytes(),
    )
    .unwrap();
    assert!(matches!(
        authenticate(&db, &peer, "first", &body),
        Err(AuthError::BadSignature)
    ));
    let body = build_body(&kp, &key, content, timestamp);
    assert!(authenticate(&db, &peer, "", &body).is_ok());
    let envelope = crate::xchacha_decrypt_raw(&key, &body).unwrap();
    let body = xchacha_encrypt_raw(&[41; 32], &envelope).unwrap();
    assert!(matches!(
        authenticate(&db, &peer, "first", &body),
        Err(AuthError::BadSignature)
    ));
    assert_eq!(
        db.with_conn(|c| c.query_row("SELECT COUNT(*) FROM chats", [], |row| row.get::<_, i64>(0)))
            .unwrap(),
        0
    );
}

#[test]
fn status_handshake_and_graphql_signatures_cannot_cross_protocols() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let (peer, key, kp) = seed_paired_peer(&db);
    let body = crate::peers::status::handshake(&key, &kp, &peer, &uuid::Uuid::new_v4().to_string())
        .unwrap();
    assert!(authenticate_status(&db, &peer, &body).is_ok());
    assert!(matches!(
        authenticate(&db, &peer, "", &body),
        Err(AuthError::BadSignature)
    ));
    let body = build_body(&kp, &key, &peer, now_ms());
    assert!(authenticate(&db, &peer, "", &body).is_ok());
    assert!(matches!(
        authenticate_status(&db, &peer, &body),
        Err(AuthError::BadSignature)
    ));
    let timestamp = now_ms();
    let signature = ed25519_sign(&kp, format!("{timestamp}{peer}").as_bytes());
    let body =
        xchacha_encrypt_raw(&key, format!("{signature}|{timestamp}|{peer}").as_bytes()).unwrap();
    assert!(matches!(
        authenticate_status(&db, &peer, &body),
        Err(AuthError::BadSignature)
    ));
}
