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
        encryption_public_key: None,
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

const RECIPIENT: &str = "recipient";

fn authenticate(db: &Db, sender: &str, channel: &str, body: &[u8]) -> Result<AuthenticatedPeer, AuthError> {
    authenticate_for(db, sender, RECIPIENT, channel, body, || Ok([41; 32]))
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
    let request = request.as_object_mut().unwrap();
    request
        .entry("query")
        .or_insert_with(|| "{ __typename }".into());
    request
        .entry("variables")
        .or_insert_with(|| plain_libs::serde_json::json!({}));
    plain_libs::serde_json::to_string(request).unwrap()
}
fn signed_payload(kp: &[u8], channel: &str, request: &str) -> String {
    let mut value: plain_libs::serde_json::Value =
        plain_libs::serde_json::from_str(request).unwrap();
    value["signature"] = "".into();
    let mut wire: SignedEnvelope = plain_libs::serde_json::from_value(value).unwrap();
    wire.signature = ed25519_sign(kp, &wire.signing_bytes((!channel.is_empty()).then_some(channel), RECIPIENT));
    plain_libs::serde_json::to_string(&wire).unwrap()
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
#[cfg(feature = "prefs")]
#[test]
fn authenticate_channel_decrypts_the_binary_body_and_checks_the_inner_signature() {
    use crate::features::prefs::{Prefs, security};
    let (db, _) = unique_db();
    let (_, peer_key, kp) = seed_paired_peer(&db);
    db.update_peer_status("peer-a", PeerStatus::Channel);
    let prefs = Prefs::in_memory();
    prefs.set("client_id", "recipient").unwrap();
    security::bootstrap(&prefs).unwrap();
    let recipient = security::encryption_public_key(&prefs).unwrap();
    let receive = |body: &[u8]| super::authenticate(&db, &prefs, "peer-a", "channel", body);
    let wire = crate::infrastructure::transport::contract::chat_item_request("{}").unwrap();
    let seal = |key: &[u8; 32], intended: &str| {
        let mut request: SignedEnvelope = plain_libs::serde_json::from_str(&wire).unwrap();
        request.message_id = Some("message-id".into());
        request.sign(&kp, Some("channel"), intended).unwrap();
        let plaintext = plain_libs::serde_json::to_vec(&request).unwrap();
        super::super::channel_request::seal(key, "channel", &plaintext).unwrap()
    };
    let (body, response_key) = seal(&recipient, "recipient");
    assert!(plain_libs::serde_json::from_slice::<plain_libs::serde_json::Value>(&body).is_err());
    let private = security::encryption_private_key(&prefs).unwrap();
    let info = super::super::http_request::aad("POST", "/peer_graphql");
    let (enc, ciphertext) = body.split_first_chunk::<32>().unwrap();
    let context = plain_libs::crypto::hpke::setup_recipient(&private, enc, &info).unwrap();
    let plaintext = context.open(b"channel", ciphertext).expect("HPKE must use the existing peer HTTP context");
    let request: SignedEnvelope = plain_libs::serde_json::from_slice(&plaintext).unwrap();
    assert_eq!(body.len(), 32 + plaintext.len() + 16);
    assert_eq!(request.message_id.as_deref(), Some("message-id"));
    assert!(!request.signature.is_empty());
    let received = receive(&body).unwrap();
    assert_eq!(received.message_id.as_deref(), Some("message-id"));
    assert_eq!(received.key, response_key);
    let (wrong_recipient, _) = seal(&recipient, "other");
    assert!(matches!(receive(&wrong_recipient), Err(AuthError::BadSignature)));
    let wrong_key = plain_libs::crypto::hpke::public_key(&[23; 32]);
    let (wrong_cipher, _) = seal(&wrong_key, "recipient");
    assert!(matches!(receive(&wrong_cipher), Err(AuthError::DecryptFailed)));
    let mut damaged = body;
    damaged[32] ^= 1;
    assert!(matches!(receive(&damaged), Err(AuthError::DecryptFailed)));
    let mut altered = request;
    altered.message_id = Some("changed".into());
    let (enc, context) = plain_libs::crypto::hpke::setup_sender(&recipient, &info).unwrap();
    let ciphertext = context.seal(b"channel", &plain_libs::serde_json::to_vec(&altered).unwrap()).unwrap();
    let reencrypted = [enc.as_slice(), &ciphertext].concat();
    assert!(matches!(receive(&reencrypted), Err(AuthError::BadSignature)));
    let old_wrapper = plain_libs::serde_json::to_vec(&plain_libs::serde_json::json!({"messageId":"message-id","sealed":"x","signature":"x"})).unwrap();
    assert!(receive(&old_wrapper).is_err());
    let old = encrypt_peer(&peer_key, wire.as_bytes()).unwrap();
    assert!(receive(&old).is_err());
    assert!(matches!(
        authenticate(&db, "peer-a", "", &build_body(&kp, &peer_key, "{}", now_ms())),
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
    let request = request_at(r#"{"query":"{ __typename }"}"#, now_ms());
    let payload = signed_payload(&kp, "first", &request);
    let old = encrypt_peer(&key, payload.as_bytes()).unwrap();
    assert!(matches!(authenticate(&db, &peer, "", &old), Err(AuthError::BadSignature)));
    assert!(matches!(authenticate(&db, &peer, "first", &old), Err(AuthError::DecryptFailed)));
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
        Err(AuthError::DecryptFailed)
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
    let mut wire: SignedEnvelope =
        plain_libs::serde_json::from_str(&signed_payload(&kp, "", &request)).unwrap();
    let mut tuple: plain_libs::serde_json::Value =
        plain_libs::serde_json::from_slice(&wire.signing_bytes(None, RECIPIENT)).unwrap();
    tuple[0] = "PEER_STATUS".into();
    wire.signature = ed25519_sign(&kp, tuple.to_string().as_bytes());
    let payload = plain_libs::serde_json::to_string(&wire).unwrap();
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
    extra["operationName"] = "unsigned".into();
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
        ids.insert(
            super::super::graphql_request::Envelope::parse(&request)
                .unwrap()
                .request_id,
        );
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
        altered.request_id = crate::features::record_id::generate();
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
fn peer_signature_covers_flat_fields_not_json_key_order() {
    let (db, _) = unique_db();
    let (id, key, kp) = seed_paired_peer(&db);
    let request = request_at(
        r#"{"query":"{ __typename }","variables":{"nested":{"a":1,"z":2}}}"#,
        now_ms(),
    );
    let payload = signed_payload(&kp, "", &request);
    let wire: SignedEnvelope = plain_libs::serde_json::from_str(&payload).unwrap();
    for tampered in [
        payload.replace("__typename", "other"),
        payload.replace(r#""z":2"#, r#""z":3"#),
    ] {
        assert!(matches!(
            authenticate(
                &db,
                &id,
                "",
                &encrypt_peer(&key, tampered.as_bytes()).unwrap()
            ),
            Err(AuthError::BadSignature)
        ));
    }
    let reordered = format!(
        r#"{{"variables":{{"nested":{{"z":2,"a":1}}}},"query":"{{ __typename }}","requestId":"{}","signature":"{}"}}"#,
        wire.request_id, wire.signature,
    );
    let body = encrypt_peer(&key, reordered.as_bytes()).unwrap();
    assert_eq!(
        authenticate(&db, &id, "", &body).unwrap().graphql_json,
        request
    );
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
fn peer_signature_matches_portable_seven_field_vectors() {
    // The fixed Ed25519 signatures are independently generated with Node crypto.
    let kp = plain_libs::crypto::ed25519_keypair_from_seed(&[0x11; 32]);
    let query = "{ __typename }";
    let request_id = "01a1cd7b-b900-7331-9331-313131313133";
    let variables = plain_libs::serde_json::from_str(r#"{"z":true,"a":[{"b":2,"a":1}]}"#).unwrap();
    let mut wire = SignedEnvelope {
        signature: String::new(),
        request_id: request_id.into(),
        query: query.into(),
        variables,
        message_id: None,
    };
    for (channel, message, canonical, signature) in [
        (None, None, r#"["PEER_REQUEST",null,"01a1cd7b-b900-7331-9331-313131313133","{ __typename }",{"a":[{"a":1,"b":2}],"z":true},"recipient",null]"#,
            "yK6/HREJVnfA2P87R2s1kcI0vs2SfVTdlBQFifudVXrLM9mlwOponOYMuWpB6/Wz9YsdqXHr6Iz6DCzhWnL7Ag=="),
        (None, Some("message"), r#"["PEER_REQUEST",null,"01a1cd7b-b900-7331-9331-313131313133","{ __typename }",{"a":[{"a":1,"b":2}],"z":true},"recipient","message"]"#,
            "/+avtUEUEhOo+89wQzYuGEY91GSqgNrCyLeNd2NRRF9fL8PP0txXTBR3AjIkbaim+KHQy2OStLmpO7DIVHDnCQ=="),
        (Some("channel"), Some("message"), r#"["PEER_REQUEST","channel","01a1cd7b-b900-7331-9331-313131313133","{ __typename }",{"a":[{"a":1,"b":2}],"z":true},"recipient","message"]"#,
            "/97foLAYNaUzGXjk6UydCl2gXGnzRnQ+Xsg6E+iIyJh5kx8w4CDarjN4xr61rq6crOBxoFRmPT1D8OlgXMVCAA=="),
    ] {
        wire.message_id = message.map(str::to_owned);
        assert_eq!(wire.signing_bytes(channel, RECIPIENT), canonical.as_bytes());
        wire.sign(&kp, channel, RECIPIENT).unwrap();
        assert_eq!(wire.signature, signature);
    }
}

#[test]
fn peer_verification_requires_the_local_recipient_and_an_unambiguous_sender_key() {
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let (sender, key, kp) = seed_paired_peer(&db);
    let body = build_body(&kp, &key, "{}", now_ms());
    assert!(matches!(
        authenticate_for(&db, &sender, "other", "", &body, || unreachable!()),
        Err(AuthError::BadSignature)
    ));
    let mut alias = peer(&db, &sender).unwrap();
    alias.id = "another-peer".into();
    db.upsert_peer(&alias);
    assert!(matches!(authenticate(&db, &alias.id, "", &body), Err(AuthError::UnknownPeer)));
    assert!(matches!(authenticate(&db, &sender, "", &body), Err(AuthError::UnknownPeer)));
}

#[test]
fn direct_send_and_verify_share_the_seven_field_signature() {
    use super::super::contract::{PeerDispatch, peer_request};
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let (sender, key, kp) = seed_paired_peer(&db);
    let target = DPeer::new(RECIPIENT, "Recipient", "", None, DeviceType::Computer);
    let dispatch = PeerDispatch {
        peer: &target, client_id: &sender, channel_id: "", key: &key, signing_key: &kp, message_id: None,
    };
    let body = peer_request("{ __typename }", plain_libs::serde_json::json!({})).unwrap();
    let (ciphertext, _) = dispatch.seal(&body).unwrap();
    assert!(authenticate(&db, &sender, "", &ciphertext).is_ok());
}

#[test]
fn message_signatures_bind_every_payload_field_and_the_conversation() {
    use plain_libs::serde_json::{json, to_vec};
    let (kp, public) = plain_libs::crypto::ed25519_generate();
    let public = plain_libs::utils::base64::base64_encode(&public);
    for channel in [None, Some("fixture-channel")] {
        let mut signed = SignedEnvelope {
            signature: String::new(),
            request_id: crate::features::record_id::generate(),
            query: "mutation { createChatItem(content: $content) { id } }".into(),
            variables: json!({"content": "fixture"}).as_object().unwrap().clone(),
            message_id: Some("fixture-message".into()),
        };
        signed.sign(&kp, channel, RECIPIENT).unwrap();
        let body = to_vec(&signed).unwrap();
        assert!(verify(&public, channel, RECIPIENT, &body).is_ok());
        assert!(matches!(verify(&public, channel, "different-device", &body), Err(AuthError::BadSignature)));
        assert!(matches!(verify(&public, Some("different-channel"), RECIPIENT, &body), Err(AuthError::BadSignature)));
        for (name, value) in [
            ("requestId", json!(crate::features::record_id::generate())),
            ("query", json!("{ __typename }")),
            ("variables", json!({"content": "changed"})),
            ("messageId", json!("different-message")),
        ] {
            let mut altered: plain_libs::serde_json::Value = plain_libs::serde_json::from_slice(&body).unwrap();
            altered[name] = value;
            assert!(matches!(verify(&public, channel, RECIPIENT, &to_vec(&altered).unwrap()), Err(AuthError::BadSignature)), "{name}");
        }
    }
}
