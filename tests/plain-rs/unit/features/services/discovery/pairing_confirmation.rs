use super::*;
use std::sync::Arc;
fn fixture(id: &str) -> (plain_libs::temporary::TempDir, crate::test_runtime::Core) {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs =
        Arc::new(crate::features::prefs::Prefs::load(&dir.path().join("system.json")).unwrap());
    let (key, _) = plain_libs::crypto::ed25519_generate();
    prefs.set("client_id", id).unwrap();
    prefs
        .set("signature_key_pair", base64_encode(&key))
        .unwrap();
    let core =
        crate::test_runtime::Core::start(dir.path(), &base64_encode(&[3; 32]), prefs).unwrap();
    (dir, core)
}
fn device() -> pairing::Device {
    pairing::Device {
        name: "Fixture".into(),
        port: 2443,
        device_type: crate::features::chat::enums::DeviceType::Phone,
        ips: vec![],
        aware_supported: false,
    }
}
fn begin(a: &CoreRuntime, b: &CoreRuntime) -> (PairingRequest, Ticket) {
    let id = b.native_client_id().unwrap();
    let key = crate::features::services::peers::peer_wire::signing_keypair(&b.prefs).unwrap();
    let target = Target {
        device_id: id,
        device_public_key: base64_encode(&key[32..]),
        device_name: "Remote".into(),
        device_ip: "127.0.0.1".into(),
        device_port: 2443,
    };
    let value = pairing::start_generation(&a.prefs, &a.pairing, target, device(), None).unwrap();
    let request: PairingRequest =
        plain_libs::serde_json::from_value(value["request"].clone()).unwrap();
    let ticket: Ticket = plain_libs::serde_json::from_value(value["ticket"].clone()).unwrap();
    a.pairing_runtime
        .confirmations
        .begin(a, &request, &ticket)
        .unwrap();
    assert_eq!(pairing::receive_request(&b.pairing, &request), Some(true));
    b.pairing_runtime
        .confirmations
        .register(b, &request)
        .unwrap();
    (request, ticket)
}
fn peer(state: &CoreRuntime, id: &str) -> Option<crate::infrastructure::db::DPeer> {
    crate::infrastructure::db::chat_store::peers::get(&state.db, id).unwrap()
}

#[tokio::test]
async fn signed_reply_commits_both_peers_and_retries_keep_original_key() {
    let (_ad, a) = fixture("a");
    let (_bd, b) = fixture("b");
    let a = a.runtime_state();
    let b = b.runtime_state();
    let (request, _) = begin(&a, &b);
    assert!(peer(&a, "b").is_none());
    assert!(peer(&b, "a").is_none());

    let (reply, _) = b
        .pairing_runtime
        .confirmations
        .respond(&b, &request, true, device())
        .unwrap();
    let b_peer = peer(&b, "a").unwrap();
    assert!(peer(&a, "b").is_none());
    let (retried, _) = b
        .pairing_runtime
        .confirmations
        .respond(&b, &request, false, device())
        .unwrap();
    assert_eq!(
        plain_libs::serde_json::to_vec(&retried).unwrap(),
        plain_libs::serde_json::to_vec(&reply).unwrap()
    );
    assert_eq!(peer(&b, "a").unwrap().updated_at, b_peer.updated_at);

    let mut forged = reply.clone();
    let PairingReply::Accept(accepted) = &mut forged else {
        panic!("Expected acceptance")
    };
    accepted.signature = base64_encode(&[0; 64]);
    assert!(
        a.pairing_runtime
            .confirmations
            .response(&a, &forged, "")
            .is_err()
    );
    assert!(peer(&a, "b").is_none());
    a.pairing_runtime
        .confirmations
        .response(&a, &retried, "")
        .unwrap();
    let a_peer = peer(&a, "b").unwrap();
    assert_eq!(a_peer.key, b_peer.key);

    a.pairing_runtime
        .confirmations
        .response(&a, &reply, "")
        .unwrap();
    assert_eq!(peer(&a, "b").unwrap().updated_at, a_peer.updated_at);
}

#[tokio::test]
async fn a_canceled_attempt_cannot_commit_a_late_reply() {
    let (_ad, a) = fixture("a");
    let (_bd, b) = fixture("b");
    let a = a.runtime_state();
    let b = b.runtime_state();
    let (request, ticket) = begin(&a, &b);
    let (reply, _) = b
        .pairing_runtime
        .confirmations
        .respond(&b, &request, true, device())
        .unwrap();
    a.pairing.cancel("b", Some(&ticket.generation)).unwrap();
    assert!(
        a.pairing_runtime
            .confirmations
            .response(&a, &reply, "")
            .is_err()
    );
    assert!(peer(&a, "b").is_none());
}

#[tokio::test]
async fn declined_reply_is_cached_and_does_not_write_peers() {
    let (_ad, a) = fixture("a");
    let (_bd, b) = fixture("b");
    let a = a.runtime_state();
    let b = b.runtime_state();
    let (request, _) = begin(&a, &b);
    let (reply, _) = b
        .pairing_runtime
        .confirmations
        .respond(&b, &request, false, device())
        .unwrap();
    let cached = b
        .pairing_runtime
        .confirmations
        .cached_response(&request)
        .unwrap()
        .unwrap();
    let (PairingReply::Reject(cached_rejection), PairingReply::Reject(rejection)) =
        (&cached, &reply)
    else {
        panic!("Expected rejection")
    };
    assert_eq!(cached_rejection.signature, rejection.signature);
    a.pairing_runtime
        .confirmations
        .response(&a, &cached, "")
        .unwrap();

    assert!(peer(&a, "b").is_none());
    assert!(peer(&b, "a").is_none());
}

#[tokio::test]
async fn failed_peer_write_retries_the_same_signed_reply() {
    let (_ad, a) = fixture("a");
    let (_bd, b) = fixture("b");
    let a = a.runtime_state();
    let b = b.runtime_state();
    let (request, _) = begin(&a, &b);
    let (reply, _) = b
        .pairing_runtime
        .confirmations
        .respond(&b, &request, true, device())
        .unwrap();
    a.db.with_conn(|c| c.execute_batch("CREATE TEMP TRIGGER fail_pair BEFORE INSERT ON peers BEGIN SELECT RAISE(FAIL, 'test failure'); END;")).unwrap();
    assert!(
        a.pairing_runtime
            .confirmations
            .response(&a, &reply, "")
            .is_err()
    );
    assert!(peer(&a, "b").is_none());
    a.db.with_conn(|c| c.execute_batch("DROP TRIGGER fail_pair"))
        .unwrap();
    a.pairing_runtime
        .confirmations
        .response(&a, &reply, "")
        .unwrap();
    assert_eq!(peer(&a, "b").unwrap().key, peer(&b, "a").unwrap().key);
}
