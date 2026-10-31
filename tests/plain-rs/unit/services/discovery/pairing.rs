use super::*;
fn device() -> Device {
    Device {
        name: "Fixture".into(),
        port: 2443,
        device_type: "PHONE".into(),
        ips: vec!["127.0.0.1".into()],
        aware_supported: false,
    }
}
fn prefs(dir: &std::path::Path, id: &str) -> Prefs {
    let prefs = Prefs::load(&dir.join(format!("{id}.json"))).unwrap();
    let (kp, _pubkey) = plain_rs::crypto::ed25519_generate();
    prefs.set("client_id", id).unwrap();
    prefs
        .set(
            "signature_key_pair",
            plain_rs::utils::base64::base64_encode(&kp),
        )
        .unwrap();
    prefs
}
fn respond(
    db: &Db,
    prefs: &Prefs,
    request: PairingRequest,
    accepted: bool,
    device: Device,
) -> Result<Value> {
    let sessions = Sessions::default();
    super::receive_request(&sessions, &request);
    super::respond(db, prefs, &sessions, request, accepted, device)
}
fn signed_cancel(prefs: &Prefs, request: &PairingRequest, to: &str) -> PairingCancel {
    let keypair = signing_keypair(prefs).unwrap();
    let mut value = PairingCancel {
        from_id: request.from_id.clone(),
        to_id: to.into(),
        request_signature: request.signature.clone(),
        signature_public_key: request.signature_public_key.clone(),
        timestamp: now_ms(),
        signature: String::new(),
    };
    value.signature = plain_rs::crypto::ed25519_sign(&keypair, value.signature_data().as_bytes());
    value
}
#[test]
fn signed_handshake_and_atomic_session_consumption_and_recipient_binding() {
    let dir = std::env::temp_dir().join(format!(
        "pairing-fixture-{}-{}",
        std::process::id(),
        now_ms()
    ));
    let a = prefs(&dir, "a");
    let b = prefs(&dir, "b");
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let sessions = Sessions::default();
    let target = || Target {
        device_id: "b".into(),
        device_name: "Fixture".into(),
        device_ip: "127.0.0.1".into(),
        device_port: 2443,
    };
    let started = start(&a, &sessions, target(), device()).unwrap();
    assert!(started.get("privateKey").is_none());
    assert_eq!(started["ticket"]["delayMs"], 90000);
    let req: PairingRequest = serde_json::from_value(started["request"].clone()).unwrap();
    assert!(security::verify_request(&req));
    let built = respond(&db, &b, req.clone(), true, device()).unwrap();
    let resp: PairingResponse = serde_json::from_value(built["response"].clone()).unwrap();
    assert!(!validate_response(&b, &resp, "b").unwrap());
    assert!(!validate_response(&a, &resp, "c").unwrap());
    let mut tampered = resp.clone();
    tampered.accepted = false;
    assert!(
        complete(&db, &a, &sessions, tampered, "")
            .unwrap()
            .is_null()
    );
    assert!(
        receive_cancel(
            &a,
            &sessions,
            PairingCancel {
                from_id: "b".into(),
                to_id: "wrong".into(),
                request_signature: String::new(),
                signature_public_key: String::new(),
                timestamp: now_ms(),
                signature: String::new()
            }
        )
        .unwrap()
        .is_null()
    );
    let finished = complete(&db, &a, &sessions, resp.clone(), "127.0.0.1").unwrap();
    assert_eq!(finished["peer"]["key"], built["peer"]["key"]);
    assert_eq!(finished["error"], "");
    assert!(complete(&db, &a, &sessions, resp, "").unwrap().is_null());
    let rejected = respond(&db, &b, req.clone(), false, device()).unwrap();
    assert!(rejected["peer"].is_null());
    assert_eq!(rejected["response"]["ecdhPublicKey"], "");
    for timestamp in [i64::MIN, i64::MAX] {
        let mut invalid = req.clone();
        invalid.timestamp = timestamp;
        assert!(respond(&db, &b, invalid, true, device()).unwrap().is_null());
    }
    let mut invalid = req;
    invalid.from_name = "forged".into();
    assert!(respond(&db, &b, invalid, true, device()).unwrap().is_null());
    let ticket = start(&a, &sessions, target(), device()).unwrap()["ticket"].clone();
    assert!(
        cancel(&a, &sessions, "b", Some("old-generation"))
            .unwrap()
            .is_null()
    );
    let cancelled = cancel(&a, &sessions, "b", ticket["generation"].as_str()).unwrap();
    assert_eq!(cancelled["cancel"]["fromId"], "a");
    assert_eq!(cancelled["cancel"]["toId"], "b");
    let (request, ecdh) = request(&a, device()).unwrap();
    let raw = ecdh.private_key_bytes();
    assert_eq!(raw.len(), 32);
    assert_eq!(
        EcdhSession::from_private_key(&raw)
            .unwrap()
            .public_key_bytes,
        plain_rs::utils::base64::base64_decode(&request.ecdh_public_key)
    );
    assert!(EcdhSession::from_private_key(&[0; 32]).is_none());
    drop(a);
    drop(b);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn sql_failure_is_a_failed_receipt_and_rejection_consumes_session_without_pairing() {
    let dir = std::env::temp_dir().join(format!(
        "pairing-failure-{}-{}",
        std::process::id(),
        now_ms()
    ));
    let a = prefs(&dir, "a");
    let b = prefs(&dir, "b");
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let sessions = Sessions::default();
    let target = || Target {
        device_id: "b".into(),
        device_name: "Fixture".into(),
        device_ip: "".into(),
        device_port: 1,
    };
    let first = start(&a, &sessions, target(), device()).unwrap();
    let req = serde_json::from_value(first["request"].clone()).unwrap();
    let (resp, _) = response(&b, req, false, device()).unwrap().unwrap();
    let rejected = complete(&db, &a, &sessions, resp.clone(), "").unwrap();
    assert!(rejected["peer"].is_null());
    assert_eq!(rejected["error"], "Pairing request was rejected");
    assert!(complete(&db, &a, &sessions, resp, "").unwrap().is_null());
    assert!(db.get_peer_by_id("b").is_none());
    let first = start(&a, &sessions, target(), device()).unwrap();
    let req = serde_json::from_value(first["request"].clone()).unwrap();
    let (resp, _) = response(&b, req, true, device()).unwrap().unwrap();
    db.with_conn(|c|c.execute_batch("CREATE TRIGGER fail_pair BEFORE INSERT ON peers BEGIN SELECT RAISE(ABORT,'fixture failure'); END;")).unwrap();
    let failed = complete(&db, &a, &sessions, resp.clone(), "").unwrap();
    assert!(failed["peer"].is_null());
    assert!(
        failed["error"]
            .as_str()
            .unwrap()
            .contains("fixture failure")
    );
    assert!(complete(&db, &a, &sessions, resp, "").unwrap().is_null());
    assert!(db.get_peer_by_id("b").is_none());
    drop(a);
    drop(b);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn incoming_requests_cancel_once_and_only_current_invitation_can_be_answered() {
    let dir = std::env::temp_dir().join(format!(
        "pairing-incoming-{}-{}",
        std::process::id(),
        now_ms()
    ));
    let a = prefs(&dir, "a");
    let b = prefs(&dir, "b");
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let sessions = Sessions::default();
    let (req, _) = request(&a, device()).unwrap();
    assert_eq!(receive_request(&sessions, &req), Some(true));
    assert_eq!(receive_request(&sessions, &req), Some(false));
    let (next, _) = request(&a, device()).unwrap();
    assert_eq!(receive_request(&sessions, &next), Some(true));
    assert!(
        super::respond(&db, &b, &sessions, req.clone(), true, device())
            .unwrap()
            .is_null()
    );
    let stale = signed_cancel(&a, &req, "b");
    assert!(receive_cancel(&b, &sessions, stale).unwrap().is_null());
    let mut forged = signed_cancel(&a, &next, "b");
    forged.signature.clear();
    assert!(receive_cancel(&b, &sessions, forged).unwrap().is_null());
    sessions.start(
        Target {
            device_id: "a".into(),
            device_name: "A".into(),
            device_ip: String::new(),
            device_port: 2443,
        },
        EcdhSession::generate(),
    );
    let cancelled = receive_cancel(&b, &sessions, signed_cancel(&a, &next, "b")).unwrap();
    assert_eq!(cancelled["deviceId"], "a");
    assert!(sessions.contains("a"));
    assert!(
        super::respond(&db, &b, &sessions, next, true, device())
            .unwrap()
            .is_null()
    );
    assert!(db.get_peer_by_id("a").is_none());
    drop(a);
    drop(b);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_captured_response_cannot_consume_or_overwrite_a_replacement_attempt() {
    let directory = tempfile::tempdir().unwrap();
    let a = prefs(directory.path(), "a");
    let b = prefs(directory.path(), "b");
    let db = Db::open(std::path::Path::new(":memory:")).unwrap();
    let sessions = Sessions::default();
    let target = || Target {
        device_id: "b".into(),
        device_name: "B".into(),
        device_ip: "127.0.0.1".into(),
        device_port: 2443,
    };
    let first = start(&a, &sessions, target(), device()).unwrap();
    let request = serde_json::from_value(first["request"].clone()).unwrap();
    let (captured, _) = response(&b, request, true, device()).unwrap().unwrap();
    let next = start(&a, &sessions, target(), device()).unwrap();
    let generation = next["ticket"]["generation"].as_str().unwrap();
    assert!(
        complete(&db, &a, &sessions, captured, "127.0.0.1")
            .unwrap()
            .is_null()
    );
    assert!(sessions.current("b", generation));
    assert!(db.get_peer_by_id("b").is_none());
    let request = serde_json::from_value(next["request"].clone()).unwrap();
    let (current, _) = response(&b, request, true, device()).unwrap().unwrap();
    assert_eq!(
        complete(&db, &a, &sessions, current, "127.0.0.1").unwrap()["peer"]["id"],
        "b"
    );
}
