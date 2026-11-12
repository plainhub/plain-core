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

fn request_at(content: &str, timestamp: i64) -> String {
    let mut bytes = *plain_libs::utils::uuid::Uuid::new_v4().as_bytes();
    bytes[..6].copy_from_slice(&timestamp.to_be_bytes()[2..]);
    bytes[6] = (bytes[6] & 15) | 0x70;
    bytes[8] = (bytes[8] & 63) | 0x80;
    let mut request: plain_libs::serde_json::Value =
        plain_libs::serde_json::from_str(content).unwrap();
    request.as_object_mut().unwrap().insert(
        "requestId".into(),
        plain_libs::utils::uuid::Uuid::from_bytes(bytes)
            .to_string()
            .into(),
    );
    request.to_string()
}
fn signed_payload(kp: &[u8], channel: &str, request: &str) -> String {
    plain_libs::serde_json::to_string(&SignedEnvelope {
        signature: ed25519_sign(kp, signature_payload(channel, request).as_bytes()),
        request: plain_libs::serde_json::value::RawValue::from_string(request.to_owned()).unwrap(),
    })
    .unwrap()
}
fn build_body(kp: &[u8], key: &[u8; 32], content: &str, timestamp: i64) -> Vec<u8> {
    encrypt_peer(
        key,
        signed_payload(kp, "", &request_at(content, timestamp)).as_bytes(),
    )
    .unwrap()
}

#[test]
fn authenticate_roundtrip_paired_peer() {
    let (db, _) = unique_db();
    let (id, key, kp) = seed_paired_peer(&db);
    let content = r#"{"query":"mutation { createChatItem(content: \"x\") { id } }"}"#;
    let request = request_at(content, now_ms());
    let body = encrypt_peer(&key, signed_payload(&kp, "", &request).as_bytes()).unwrap();
    let authed = authenticate(&db, &id, "", &body).unwrap();
    assert_eq!(authed.peer.id, id);
    assert_eq!(authed.graphql_json, request);
    assert_eq!(authed.key, key.to_vec());
    assert_eq!(
        authed.timestamp,
        GraphqlEnvelope::parse(&request)
            .unwrap()
            .id()
            .unwrap()
            .timestamp()
    );
}
#[test]
fn authenticate_rejects_unknown_peer_and_unpaired() {
    let (db, _) = unique_db();
    let (_, key, kp) = seed_paired_peer(&db);
    let body = build_body(&kp, &key, "{}", now_ms());
    assert!(matches!(
        authenticate(&db, "ghost", "", &body),
        Err(AuthError::UnknownPeer)
    ));
    db.update_peer_status("peer-a", PeerStatus::Channel);
    assert!(matches!(
        authenticate(&db, "peer-a", "", &body),
        Err(AuthError::NotPaired)
    ));
}
#[test]
fn authenticate_rejects_wrong_key_and_bad_signature() {
    let (db, _) = unique_db();
    let (id, key, kp) = seed_paired_peer(&db);
    assert!(matches!(
        authenticate(&db, &id, "", &build_body(&kp, &[1; 32], "{}", now_ms())),
        Err(AuthError::DecryptFailed)
    ));
    let (other, _) = ed25519_generate();
    assert!(matches!(
        authenticate(&db, &id, "", &build_body(&other, &key, "{}", now_ms())),
        Err(AuthError::BadSignature)
    ));
}
#[test]
fn expired_uuid_ids_and_missing_fields_are_rejected() {
    let (db, _) = unique_db();
    let (id, key, kp) = seed_paired_peer(&db);
    assert!(matches!(
        authenticate(
            &db,
            &id,
            "",
            &build_body(&kp, &key, "{}", now_ms() - 360_000)
        ),
        Err(AuthError::TimestampExpired)
    ));
    for plain in [
        r#"{"request":{}}"#,
        r#"{"signature":"","request":{}}"#,
        "signature|123|nonce|{}",
    ] {
        let body = encrypt_peer(&key, plain.as_bytes()).unwrap();
        assert!(matches!(
            authenticate(&db, &id, "", &body),
            Err(AuthError::MissingFields)
        ));
    }
}
#[test]
fn authenticate_channel_message_uses_channel_key_without_paired_check() {
    let (db, _) = unique_db();
    let (_, peer_key, kp) = seed_paired_peer(&db);
    db.update_peer_status("peer-a", PeerStatus::Channel);
    let channel_key = [42; 32];
    let mut channel = crate::infrastructure::db::DChannel::new("channel", "owner");
    channel.id = "ch-1".into();
    channel.key = base64_encode(&channel_key);
    crate::infrastructure::db::chat_store::channels::save(
        &db,
        &[channel],
        crate::infrastructure::db::chat_store::SaveMode::Insert,
    )
    .unwrap();
    let request = request_at(r#"{"query":"{ __typename }"}"#, now_ms());
    let body = encrypt_peer(
        &channel_key,
        signed_payload(&kp, "ch-1", &request).as_bytes(),
    )
    .unwrap();
    let auth = authenticate(&db, "peer-a", "ch-1", &body).unwrap();
    assert_eq!(auth.graphql_json, request);
    assert_eq!(auth.key, channel_key.to_vec());
    crate::infrastructure::db::chat_store::channels::delete(&db, &["ch-1".into()]).unwrap();
    assert!(matches!(
        authenticate(&db, "peer-a", "ch-1", &body),
        Err(AuthError::NoChannelKey)
    ));
    assert!(matches!(
        authenticate(
            &db,
            "peer-a",
            "",
            &build_body(&kp, &peer_key, "{}", now_ms())
        ),
        Err(AuthError::NotPaired)
    ));
}
#[test]
fn extreme_uuid_times_and_malformed_envelopes_do_not_panic_and_storage_failures_are_real() {
    let (db, _) = unique_db();
    let (id, key, kp) = seed_paired_peer(&db);
    for timestamp in [0, (1i64 << 48) - 1] {
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
    let channels: Vec<_> = [("first", [41; 32]), ("second", [42; 32])]
        .into_iter()
        .map(|(id, key)| {
            let mut c = crate::infrastructure::db::DChannel::new(id, "owner");
            c.id = id.into();
            c.key = base64_encode(&key);
            c
        })
        .collect();
    crate::infrastructure::db::chat_store::channels::save(
        &db,
        &channels,
        crate::infrastructure::db::chat_store::SaveMode::Insert,
    )
    .unwrap();
    let request = request_at(r#"{"query":"{ __typename }"}"#, now_ms());
    let payload = signed_payload(&kp, "first", &request);
    assert!(
        authenticate(
            &db,
            &peer,
            "first",
            &encrypt_peer(&[41; 32], payload.as_bytes()).unwrap()
        )
        .is_ok()
    );
    for (channel, key) in [("second", vec![42; 32]), ("", key.to_vec())] {
        assert!(matches!(
            authenticate(
                &db,
                &peer,
                channel,
                &encrypt_peer(&key, payload.as_bytes()).unwrap()
            ),
            Err(AuthError::BadSignature)
        ));
    }
    let mut legacy: SignedEnvelope = plain_libs::serde_json::from_str(&payload).unwrap();
    legacy.signature = ed25519_sign(&kp, format!("{}{request}", now_ms()).as_bytes());
    assert!(matches!(
        authenticate(
            &db,
            &peer,
            "first",
            &encrypt_peer(
                &[41; 32],
                plain_libs::serde_json::to_string(&legacy)
                    .unwrap()
                    .as_bytes()
            )
            .unwrap()
        ),
        Err(AuthError::BadSignature)
    ));
    let direct = signed_payload(&kp, "", &request_at("{}", now_ms()));
    assert!(
        authenticate(
            &db,
            &peer,
            "",
            &encrypt_peer(&key, direct.as_bytes()).unwrap()
        )
        .is_ok()
    );
    assert!(matches!(
        authenticate(
            &db,
            &peer,
            "first",
            &encrypt_peer(&[41; 32], direct.as_bytes()).unwrap()
        ),
        Err(AuthError::BadSignature)
    ));
    assert_eq!(
        db.with_conn(|c| c.query_row("SELECT COUNT(*) FROM chats", [], |r| r.get::<_, i64>(0)))
            .unwrap(),
        0
    );
}
#[test]
fn graphql_rejects_status_signature_purpose() {
    let (db, _) = unique_db();
    let (id, key, kp) = seed_paired_peer(&db);
    let request = request_at("{}", now_ms());
    let wrong_purpose = plain_libs::serde_json::json!(["PEER_STATUS", "", request]).to_string();
    let signature = ed25519_sign(&kp, wrong_purpose.as_bytes());
    let payload = plain_libs::serde_json::json!({
        "signature": signature,
        "request": plain_libs::serde_json::from_str::<plain_libs::serde_json::Value>(&request).unwrap(),
    }).to_string();
    assert!(matches!(
        authenticate(
            &db,
            &id,
            "",
            &encrypt_peer(&key, payload.as_bytes()).unwrap()
        ),
        Err(AuthError::BadSignature)
    ));
}
#[test]
fn peer_request_rejects_unsigned_outer_extras_and_wrong_route_aad() {
    let (db, _) = unique_db();
    let (id, key, kp) = seed_paired_peer(&db);
    let request = request_at("{}", now_ms());
    let payload = signed_payload(&kp, "", &request);
    for (method, endpoint) in [
        ("GET", "/peer_graphql"),
        ("POST", "/graphql"),
        ("POST", "/guest_graphql"),
    ] {
        let body = plain_libs::crypto::xchacha_encrypt_raw_with_aad(
            &key,
            payload.as_bytes(),
            &crate::infrastructure::transport::http_request::aad(method, endpoint),
        )
        .unwrap();
        assert!(matches!(
            authenticate(&db, &id, "", &body),
            Err(AuthError::DecryptFailed)
        ));
    }
    let mut extra: plain_libs::serde_json::Value =
        plain_libs::serde_json::from_str(&payload).unwrap();
    extra["requestId"] = GraphqlEnvelope::parse(&request).unwrap().request_id.into();
    for plain in [
        request,
        format!("{}|{NONCE}|{{}}", now_ms()),
        extra.to_string(),
        format!("\u{3}{payload}"),
    ] {
        assert!(matches!(
            authenticate(&db, &id, "", &encrypt_peer(&key, plain.as_bytes()).unwrap()),
            Err(AuthError::MissingFields)
        ));
    }
    let body = encrypt_peer(&key, payload.as_bytes()).unwrap();
    assert!(authenticate(&db, &id, "", &body).is_ok());
    assert!(matches!(
        authenticate(
            &db,
            &id,
            "",
            &encrypt_peer(&key, payload.as_bytes()).unwrap()
        ),
        Err(AuthError::Replay)
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
fn same_millisecond_requests_have_distinct_signed_uuid_ids_and_tampering_is_rejected() {
    let (db, _) = unique_db();
    let (id, key, kp) = seed_paired_peer(&db);
    let timestamp = now_ms();
    let mut ids = std::collections::HashSet::new();
    for _ in 0..2 {
        let request = request_at("{}", timestamp);
        ids.insert(GraphqlEnvelope::parse(&request).unwrap().request_id);
        let payload = signed_payload(&kp, "", &request);
        assert!(
            authenticate(
                &db,
                &id,
                "",
                &encrypt_peer(&key, payload.as_bytes()).unwrap()
            )
            .is_ok()
        );
        let mut altered: SignedEnvelope = plain_libs::serde_json::from_str(&payload).unwrap();
        altered.request =
            plain_libs::serde_json::value::RawValue::from_string(request_at("{}", timestamp))
                .unwrap();
        assert!(matches!(
            authenticate(
                &db,
                &id,
                "",
                &encrypt_peer(
                    &key,
                    plain_libs::serde_json::to_string(&altered)
                        .unwrap()
                        .as_bytes()
                )
                .unwrap()
            ),
            Err(AuthError::BadSignature)
        ));
    }
    assert_eq!(ids.len(), 2);
}
#[test]
fn signatures_cover_exact_request_bytes_including_whitespace_and_numbers() {
    let (db, _) = unique_db();
    let (id, key, kp) = seed_paired_peer(&db);
    let rid = GraphqlEnvelope::parse(&request_at("{}", now_ms()))
        .unwrap()
        .request_id;
    let request = format!(
        "{{ \"requestId\":\"{rid}\", \"query\":\"{{ __typename }}\", \"variables\":{{\"number\":1e+30}} }}"
    );
    let payload = signed_payload(&kp, "", &request);
    let body = encrypt_peer(&key, payload.as_bytes()).unwrap();
    assert_eq!(
        authenticate(&db, &id, "", &body).unwrap().graphql_json,
        request
    );
    let altered = payload.replace("1e+30", "1e30");
    assert_ne!(payload, altered);
    assert!(matches!(
        authenticate(
            &db,
            &id,
            "",
            &encrypt_peer(&key, altered.as_bytes()).unwrap()
        ),
        Err(AuthError::BadSignature)
    ));
}
#[test]
fn graphql_freshness_is_one_minute() {
    let (db, _) = unique_db();
    let (id, key, kp) = seed_paired_peer(&db);
    let now = now_ms();
    for offset in [-61_000, 61_000] {
        assert!(matches!(
            authenticate(&db, &id, "", &build_body(&kp, &key, "{}", now + offset)),
            Err(AuthError::TimestampExpired)
        ));
    }
    for offset in [-59_000, 59_000] {
        assert!(authenticate(&db, &id, "", &build_body(&kp, &key, "{}", now + offset)).is_ok());
    }
}
#[test]
fn peer_signature_has_explicit_leading_purpose() {
    let graphql: plain_libs::serde_json::Value =
        plain_libs::serde_json::from_str(&signature_payload("channel", "request JSON")).unwrap();
    assert_eq!(
        graphql,
        plain_libs::serde_json::json!(["PEER_REQUEST", "channel", "request JSON"])
    );
}
