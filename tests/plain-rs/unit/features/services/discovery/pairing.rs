use super::*;
fn device() -> Device {
    Device {
        name: "Fixture".into(),
        port: 2443,
        device_type: crate::features::chat::enums::DeviceType::Phone,
        ips: vec!["127.0.0.1".into()],
        aware_supported: false,
    }
}
fn prefs(dir: &std::path::Path, id: &str) -> Prefs {
    let prefs = Prefs::load(&dir.join(format!("{id}.json"))).unwrap();
    let (kp, _pubkey) = plain_libs::crypto::ed25519_generate();
    prefs.set("client_id", id).unwrap();
    prefs
        .set(
            "signature_key_pair",
            plain_libs::utils::base64::base64_encode(&kp),
        )
        .unwrap();
    prefs
}
fn signed_cancel(prefs: &Prefs, request: &PairingRequest, to: &str) -> PairingCancel {
    let keypair = signing_keypair(prefs).unwrap();
    let mut value = PairingCancel {
        from_id: request.from_id.clone(),
        to_id: to.into(),
        request_signature: request.payload.signature.clone(),
        signature_public_key: request.payload.signature_public_key.clone(),
        timestamp: now_ms(),
        signature: String::new(),
    };
    value.signature = plain_libs::crypto::ed25519_sign(&keypair, value.signature_data().as_bytes());
    value
}

fn target(prefs: &Prefs, id: &str) -> Target {
    Target {
        device_id: id.into(),
        device_name: id.into(),
        device_ip: "127.0.0.1".into(),
        device_port: 2443,
        device_public_key: plain_libs::utils::base64::base64_encode(
            &signing_keypair(prefs).unwrap()[32..],
        ),
    }
}
#[test]
fn signed_request_response_bind_recipient_identity_and_ecdh() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let a = prefs(dir.path(), "a");
    let b = prefs(dir.path(), "b");
    let c = prefs(dir.path(), "c");
    let (req, ak) = request(&a, device(), &target(&b, "b")).unwrap();
    assert!(security::verify_request(&req));
    let (reply, bk) = response(&b, req.clone(), true, device()).unwrap().unwrap();
    let PairingReply::Accept(reply) = reply else { panic!("Expected acceptance") };
    assert!(security::verify_accept(
        &reply,
        &target(&b, "b").device_public_key
    ));
    assert!(!security::verify_accept(
        &reply,
        &target(&c, "c").device_public_key
    ));
    assert!(response(&c, req.clone(), true, device()).is_err());
    assert_eq!(
        ak.compute_shared_key(&plain_libs::utils::base64::base64_decode(
            &reply.ecdh_public_key
        )),
        bk.unwrap()
            .compute_shared_key(&plain_libs::utils::base64::base64_decode(
                &req.payload.ecdh_public_key
            ))
    );
    let mut forged = req.clone();
    forged.target_public_key = target(&c, "c").device_public_key;
    assert!(!security::verify_request(&forged));
    let mut forged = reply;
    forged.ecdh_public_key.clear();
    assert!(!security::verify_accept(
        &forged,
        &target(&b, "b").device_public_key
    ));
    let (denied, key) = response(&b, req.clone(), false, device()).unwrap().unwrap();
    let PairingReply::Reject(denied) = denied else { panic!("Expected rejection") };
    assert_eq!(denied.request_id, req.payload.request_id());
    assert!(security::verify_reject(&denied, &target(&b, "b").device_public_key));
    assert!(!security::verify_reject(&denied, &target(&c, "c").device_public_key));
    assert_eq!(plain_libs::serde_json::to_value(&denied).unwrap().as_object().unwrap().len(), 2);
    assert!(key.is_none());
    for timestamp in [i64::MIN, i64::MAX] {
        let mut bad = req.clone();
        bad.payload.timestamp = timestamp;
        assert!(response(&b, bad, true, device()).unwrap().is_none());
    }
}
#[test]
fn incoming_cancel_matches_current_signed_invitation() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let a = prefs(dir.path(), "a");
    let b = prefs(dir.path(), "b");
    let sessions = Sessions::default();
    let (old, _) = request(&a, device(), &target(&b, "b")).unwrap();
    assert_eq!(receive_request(&sessions, &old), Some(true));
    assert_eq!(receive_request(&sessions, &old), Some(false));
    let (new, _) = request(&a, device(), &target(&b, "b")).unwrap();
    assert_eq!(receive_request(&sessions, &new), Some(true));
    assert!(
        receive_cancel(&b, &sessions, signed_cancel(&a, &old, "b"))
            .unwrap()
            .is_null()
    );
    let mut forged = signed_cancel(&a, &new, "b");
    forged.signature.clear();
    assert!(receive_cancel(&b, &sessions, forged).unwrap().is_null());
    assert_eq!(
        receive_cancel(&b, &sessions, signed_cancel(&a, &new, "b")).unwrap()["deviceId"],
        "a"
    );
    assert!(
        sessions
            .incoming_target("a", &new.payload.signature)
            .is_none()
    );
}
#[test]
fn cancellation_is_bound_to_generation_and_private_key_never_serialized() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let a = prefs(dir.path(), "a");
    let b = prefs(dir.path(), "b");
    let sessions = Sessions::default();
    let value = start(&a, &sessions, target(&b, "b"), device()).unwrap();
    assert!(value.get("privateKey").is_none());
    assert!(cancel(&a, &sessions, "b", Some("stale")).unwrap().is_null());
    assert_eq!(
        cancel(&a, &sessions, "b", value["ticket"]["generation"].as_str()).unwrap()["cancel"]["toId"],
        "b"
    );
    assert!(EcdhSession::from_private_key(&[0; 32]).is_none());
}
