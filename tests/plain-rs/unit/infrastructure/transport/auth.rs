use super::*;
use crate::features::chat::enums::DeviceType;
use crate::features::chat::enums::PeerStatus;
use crate::infrastructure::db::DPeer;
use crate::infrastructure::db::Db;
use plain_libs::crypto::ed25519_generate;
use plain_libs::crypto::ed25519_sign;
fn encrypt_peer(key: &[u8], payload: &[u8]) -> Option<Vec<u8>> {
    crate::infrastructure::transport::http_request::encrypt_raw(
        key,
        "POST",
        "/peer_graphql",
        payload,
    )
}
use plain_libs::utils::base64::base64_encode;
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

const NONCE: &str = "ad3d447d-9eb5-4f25-a415-726b887159ca";

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// Build an encrypted+signed request body exactly like the sender side
/// (`deliver_to_peer` wire format).
fn build_body(kp: &[u8], key: &[u8; 32], graphql_json: &str, ts: i64) -> Vec<u8> {
    let sig_data = signature_payload(ts, "", NONCE, graphql_json);
    let signature = ed25519_sign(kp, sig_data.as_bytes());
    let payload = format!("{signature}|{ts}|{NONCE}|{graphql_json}");
    encrypt_peer(key, payload.as_bytes()).expect("encrypt")
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
    let body = encrypt_peer(&key, payload.as_bytes()).unwrap();
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
    let mut channel = crate::infrastructure::db::DChannel::new("channel", "owner");
    channel.id = "ch-1".into();
    channel.key = base64_encode(&channel_key);
    crate::infrastructure::db::chat_store::channels::save(
        &db,
        &[channel],
        crate::infrastructure::db::chat_store::SaveMode::Insert,
    )
    .unwrap();
    let graphql_json =
        r#"{"query":"mutation { channelSystemMessage(type: INVITE, payload: \"{}\") }"}"#;
    let timestamp = now_ms();
    let signature = ed25519_sign(
        &kp,
        signature_payload(timestamp, "ch-1", NONCE, graphql_json).as_bytes(),
    );
    let body = encrypt_peer(
        &channel_key,
        format!("{signature}|{timestamp}|{NONCE}|{graphql_json}").as_bytes(),
    )
    .unwrap();

    // With c-cid set, the channel key decrypts and no paired check fires.
    let authed = authenticate(&db, "peer-a", "ch-1", &body).expect("channel auth ok");
    assert_eq!(authed.graphql_json, graphql_json);
    assert_eq!(authed.key, channel_key.to_vec());

    // Without a cached channel key → NoChannelKey.
    crate::infrastructure::db::chat_store::channels::delete(&db, &["ch-1".into()]).unwrap();
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
    let malformed = encrypt_peer(&key, b"signature|not-a-number|{}").unwrap();
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
        let mut channel = crate::infrastructure::db::DChannel::new(id, "owner");
        channel.id = id.into();
        channel.key = base64_encode(&key);
        channels.push(channel);
    }
    crate::infrastructure::db::chat_store::channels::save(
        &db,
        &channels,
        crate::infrastructure::db::chat_store::SaveMode::Insert,
    )
    .unwrap();
    let content = r#"{"query":"mutation($content:String!){createChatItem(content:$content){id}}","variables":{"content":"{\"type\":\"TEXT\",\"value\":{\"text\":\"context\"}}"}}"#;
    let timestamp = now_ms();
    let signature = ed25519_sign(
        &kp,
        signature_payload(timestamp, "first", NONCE, content).as_bytes(),
    );
    let envelope = format!("{signature}|{timestamp}|{NONCE}|{content}");
    let first = encrypt_peer(&[41; 32], envelope.as_bytes()).unwrap();
    assert!(authenticate(&db, &peer, "first", &first).is_ok());
    for (channel, key) in [("second", vec![42; 32]), ("", key.to_vec())] {
        let body = encrypt_peer(&key, envelope.as_bytes()).unwrap();
        assert!(matches!(
            authenticate(&db, &peer, channel, &body),
            Err(AuthError::BadSignature)
        ));
    }
    let legacy = ed25519_sign(&kp, format!("{timestamp}{content}").as_bytes());
    let body = encrypt_peer(
        &[41; 32],
        format!("{legacy}|{timestamp}|{NONCE}|{content}").as_bytes(),
    )
    .unwrap();
    assert!(matches!(
        authenticate(&db, &peer, "first", &body),
        Err(AuthError::BadSignature)
    ));
    let body = build_body(&kp, &key, content, timestamp);
    assert!(authenticate(&db, &peer, "", &body).is_ok());
    let envelope = crate::infrastructure::transport::http_request::decrypt_raw(
        &key,
        "POST",
        "/peer_graphql",
        &body,
    )
    .unwrap();
    let body = encrypt_peer(&[41; 32], &envelope).unwrap();
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
    let body = crate::features::peers::status::handshake(
        &key,
        &kp,
        &peer,
        &plain_libs::utils::uuid::Uuid::new_v4().into_string(),
    )
    .unwrap();
    assert!(authenticate_status(&db, &peer, &body).is_ok());
    assert!(matches!(
        authenticate(&db, &peer, "", &body),
        Err(AuthError::DecryptFailed)
    ));
    let body = build_body(&kp, &key, &peer, now_ms());
    assert!(authenticate(&db, &peer, "", &body).is_ok());
    assert!(matches!(
        authenticate_status(&db, &peer, &body),
        Err(AuthError::DecryptFailed)
    ));
    let timestamp = now_ms();
    let signature = ed25519_sign(&kp, format!("{timestamp}{peer}").as_bytes());
    let body = plain_libs::crypto::xchacha_encrypt_raw(
        &key,
        format!("{signature}|{timestamp}|{peer}").as_bytes(),
    )
    .unwrap();
    assert!(matches!(
        authenticate_status(&db, &peer, &body),
        Err(AuthError::BadSignature)
    ));
}

#[test]
fn peer_request_rejects_wrong_http_context_and_unbound_ciphertext() {
    let (db, _path) = unique_db();
    let (id, key, kp) = seed_paired_peer(&db);
    let now = now_ms();
    let content = "{}";
    let signature = ed25519_sign(&kp, signature_payload(now, "", NONCE, content).as_bytes());
    let payload = format!("{signature}|{now}|{NONCE}|{content}");
    for (method, endpoint) in [
        ("GET", "/peer_graphql"),
        ("POST", "/graphql"),
        ("POST", "/guest_graphql"),
    ] {
        let body = crate::infrastructure::transport::http_request::encrypt_raw(
            &key,
            method,
            endpoint,
            payload.as_bytes(),
        )
        .unwrap();
        assert!(matches!(
            authenticate(&db, &id, "", &body),
            Err(AuthError::DecryptFailed)
        ));
    }
    let body = plain_libs::crypto::xchacha_encrypt_raw(&key, payload.as_bytes()).unwrap();
    assert!(matches!(
        authenticate(&db, &id, "", &body),
        Err(AuthError::DecryptFailed)
    ));
}

#[test]
fn signed_peer_replay_is_rejected_after_reencryption_and_database_reopen() {
    let (db, path) = unique_db();
    let (id, key, kp) = seed_paired_peer(&db);
    let body = build_body(&kp, &key, "{}", now_ms());
    let plaintext = crate::infrastructure::transport::http_request::decrypt_raw(
        &key,
        "POST",
        "/peer_graphql",
        &body,
    )
    .unwrap();
    assert!(authenticate(&db, &id, "", &body).is_ok());
    assert!(matches!(
        authenticate(&db, &id, "", &body),
        Err(AuthError::Replay)
    ));
    let reencrypted = encrypt_peer(&key, &plaintext).unwrap();
    assert_ne!(reencrypted, body);
    assert!(matches!(
        authenticate(&db, &id, "", &reencrypted),
        Err(AuthError::Replay)
    ));
    drop(db);
    let reopened = Db::open(&path).unwrap();
    assert!(matches!(
        authenticate(&reopened, &id, "", &reencrypted),
        Err(AuthError::Replay)
    ));
}

#[test]
fn same_millisecond_identical_requests_have_distinct_signed_nonces() {
    let (db, _path) = unique_db();
    let (id, key, kp) = seed_paired_peer(&db);
    let timestamp = now_ms();
    let content = "{}";
    for _ in 0..2 {
        let nonce = plain_libs::utils::uuid::Uuid::new_v4().into_string();
        let signature = ed25519_sign(
            &kp,
            signature_payload(timestamp, "", &nonce, content).as_bytes(),
        );
        let body = encrypt_peer(
            &key,
            format!("{signature}|{timestamp}|{nonce}|{content}").as_bytes(),
        )
        .unwrap();
        assert!(authenticate(&db, &id, "", &body).is_ok());
        let altered = plain_libs::utils::uuid::Uuid::new_v4().into_string();
        let tampered = encrypt_peer(
            &key,
            format!("{signature}|{timestamp}|{altered}|{content}").as_bytes(),
        )
        .unwrap();
        assert!(matches!(
            authenticate(&db, &id, "", &tampered),
            Err(AuthError::BadSignature)
        ));
    }
}

#[test]
fn graphql_freshness_is_one_minute_without_shortening_status_window() {
    let (db, _path) = unique_db();
    let (id, key, kp) = seed_paired_peer(&db);
    let now = now_ms();
    for offset in [-61_000, 61_000] {
        let body = build_body(&kp, &key, "{}", now + offset);
        assert!(matches!(
            authenticate(&db, &id, "", &body),
            Err(AuthError::TimestampExpired)
        ));
    }
    for offset in [-59_000, 59_000] {
        let body = build_body(&kp, &key, "{}", now + offset);
        assert!(authenticate(&db, &id, "", &body).is_ok());
    }
    let timestamp = now - 120_000;
    let content = "status fixture";
    let signature = ed25519_sign(&kp, status_signature_payload(timestamp, content).as_bytes());
    let body = plain_libs::crypto::xchacha_encrypt_raw(
        &key,
        format!("{signature}|{timestamp}|{content}").as_bytes(),
    )
    .unwrap();
    assert!(authenticate_status(&db, &id, &body).is_ok());
}
