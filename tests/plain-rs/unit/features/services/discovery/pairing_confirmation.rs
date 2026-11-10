use super::*;
use crate::features::services::discovery::pairing_attempts::Mode;
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
    let (attempt, _, _) = a
        .pairing_runtime
        .attempts
        .begin(Some(a), Some(id.clone()), None, Mode::Chat)
        .unwrap();
    let key = crate::features::services::peers::peer_wire::signing_keypair(&b.prefs).unwrap();
    let target = Target {
        device_id: id,
        device_public_key: base64_encode(&key[32..]),
        device_name: "Remote".into(),
        device_ip: "127.0.0.1".into(),
        device_port: 2443,
    };
    let value = pairing::start_generation(
        &a.prefs,
        &a.pairing,
        target,
        device(),
        Some(attempt.generation),
    )
    .unwrap();
    let request: PairingRequest = serde_json::from_value(value["request"].clone()).unwrap();
    let ticket: Ticket = serde_json::from_value(value["ticket"].clone()).unwrap();
    a.pairing_runtime
        .confirmations
        .begin(&request, &ticket)
        .unwrap();
    assert_eq!(pairing::receive_request(&b.pairing, &request), Some(true));
    let target = b
        .pairing
        .incoming_target(&request.from_id, &request.payload.signature)
        .unwrap();
    b.pairing_runtime
        .confirmations
        .register(b, &request, target)
        .unwrap();
    (request, ticket)
}
fn verification(action: Action) -> Verification {
    match action {
        Action::Send {
            message: Message::PairVerification(v),
            ..
        }
        | Action::Notify {
            message: Message::PairVerification(v),
            ..
        } => v,
        _ => panic!("expected verification"),
    }
}
fn peer(state: &CoreRuntime, id: &str) -> Option<crate::infrastructure::db::DPeer> {
    crate::infrastructure::db::chat_store::peers::get(&state.db, id).unwrap()
}
#[tokio::test]
async fn four_flights_require_both_approvals_and_new_key_possession() {
    let (_ad, a) = fixture("a");
    let (_bd, b) = fixture("b");
    let a = &a.runtime_state();
    let b = &b.runtime_state();
    let (request, ticket) = begin(a, b);
    let digest = identity_digest(&request).unwrap();
    assert_eq!(digest.len(), 64);
    let response = b
        .pairing_runtime
        .confirmations
        .respond(b, &request, true, device())
        .unwrap();
    assert!(peer(a, "b").is_none());
    assert!(peer(b, "a").is_none());
    assert!(
        a.pairing_runtime
            .confirmations
            .response(a, &response, "")
            .unwrap()
            .is_none()
    );
    assert!(peer(a, "b").is_none());
    assert!(peer(b, "a").is_none());
    let finish = verification(
        a.pairing_runtime
            .confirmations
            .confirm(a, &ticket.generation)
            .unwrap(),
    );
    let serialized = serde_json::to_string(&finish).unwrap();
    let ack = verification(b.pairing_runtime.confirmations.process(b, finish).unwrap());
    assert!(peer(a, "b").is_none());
    assert!(peer(b, "a").is_some());
    let cached = verification(
        b.pairing_runtime
            .confirmations
            .process(b, serde_json::from_str(&serialized).unwrap())
            .unwrap(),
    );
    assert_eq!(
        serde_json::to_string(&cached).unwrap(),
        serde_json::to_string(&ack).unwrap()
    );
    a.pairing_runtime.confirmations.process(a, ack).unwrap();
    assert_eq!(peer(a, "b").unwrap().key, peer(b, "a").unwrap().key);
}
#[tokio::test]
async fn replayed_request_and_reflected_finish_do_not_activate() {
    let (_ad, a) = fixture("a");
    let (_bd, b) = fixture("b");
    let a = &a.runtime_state();
    let b = &b.runtime_state();
    let (request, ticket) = begin(a, b);
    let response = b
        .pairing_runtime
        .confirmations
        .respond(b, &request, true, device())
        .unwrap();
    a.pairing_runtime
        .confirmations
        .response(a, &response, "")
        .unwrap();
    let Verification::Finish {
        request_signature,
        proof,
    } = verification(
        a.pairing_runtime
            .confirmations
            .confirm(a, &ticket.generation)
            .unwrap(),
    )
    else {
        panic!()
    };
    assert!(
        a.pairing_runtime
            .confirmations
            .process(
                a,
                Verification::Ack {
                    request_signature: request_signature.clone(),
                    proof: proof.clone()
                }
            )
            .is_err()
    );
    assert!(peer(a, "b").is_none());
    let target = b
        .pairing
        .incoming_target(&request.from_id, &request.payload.signature)
        .unwrap();
    assert!(
        !b.pairing_runtime
            .confirmations
            .register(b, &request, target)
            .unwrap()
    );
    assert!(peer(b, "a").is_none());
    b.pairing_runtime
        .confirmations
        .process(
            b,
            Verification::Finish {
                request_signature,
                proof,
            },
        )
        .unwrap();
}
#[tokio::test]
async fn cancellation_prevents_late_finish_and_preserves_active_key() {
    let (_ad, a) = fixture("a");
    let (_bd, b) = fixture("b");
    let a = &a.runtime_state();
    let b = &b.runtime_state();
    let (request, ticket) = begin(a, b);
    let response = b
        .pairing_runtime
        .confirmations
        .respond(b, &request, true, device())
        .unwrap();
    a.pairing_runtime
        .confirmations
        .response(a, &response, "")
        .unwrap();
    let finish = verification(
        a.pairing_runtime
            .confirmations
            .confirm(a, &ticket.generation)
            .unwrap(),
    );
    b.pairing
        .take_incoming(&request.from_id, &request.payload.signature);
    assert!(b.pairing_runtime.confirmations.process(b, finish).is_err());
    assert!(peer(b, "a").is_none());
}
#[tokio::test]
async fn transcript_and_roles_change_key_and_full_digest() {
    let (_ad, a) = fixture("a");
    let (_bd, b) = fixture("b");
    let (request, _) = begin(&a.runtime_state(), &b.runtime_state());
    let response = b
        .runtime_state()
        .pairing_runtime
        .confirmations
        .respond(&b.runtime_state(), &request, true, device())
        .unwrap();
    let channel = Channel::new(&[3; 32], &request, &response);
    let mut changed = response.clone();
    changed.aware_supported = !changed.aware_supported;
    assert_ne!(channel.key, Channel::new(&[3; 32], &request, &changed).key);
    let mut swapped = request.clone();
    std::mem::swap(
        &mut swapped.payload.signature_public_key,
        &mut swapped.target_public_key,
    );
    assert_ne!(
        identity_digest(&request).unwrap(),
        identity_digest(&swapped).unwrap()
    );
}

#[tokio::test]
async fn database_failure_keeps_old_active_key_and_confirmation_retryable() {
    let (_ad, a) = fixture("a");
    let (_bd, b) = fixture("b");
    let a = a.runtime_state();
    let b = b.runtime_state();
    let public = crate::features::services::peers::peer_wire::signing_keypair(&a.prefs).unwrap();
    peer_store::save(
        &b.db,
        Facts {
            id: "a".into(),
            name: "Old".into(),
            ips: vec![],
            port: 2443,
            device_type: crate::features::chat::enums::DeviceType::Phone,
            key: base64_encode(&[42; 32]),
            public_key: base64_encode(&public[32..]),
        },
    )
    .unwrap();
    let (request, ticket) = begin(&a, &b);
    let response = b
        .pairing_runtime
        .confirmations
        .respond(&b, &request, true, device())
        .unwrap();
    assert_eq!(peer(&b, "a").unwrap().key, base64_encode(&[42; 32]));
    a.pairing_runtime
        .confirmations
        .response(&a, &response, "")
        .unwrap();
    let finish = verification(
        a.pairing_runtime
            .confirmations
            .confirm(&a, &ticket.generation)
            .unwrap(),
    );
    let wire = serde_json::to_string(&finish).unwrap();
    b.db.with_conn(|c|c.execute_batch("CREATE TRIGGER reject_pair BEFORE UPDATE ON peers BEGIN SELECT RAISE(ABORT, 'fixture'); END;")).unwrap();
    assert!(b.pairing_runtime.confirmations.process(&b, finish).is_err());
    assert_eq!(peer(&b, "a").unwrap().key, base64_encode(&[42; 32]));
    b.db.with_conn(|c| c.execute_batch("DROP TRIGGER reject_pair;"))
        .unwrap();
    let ack = verification(
        b.pairing_runtime
            .confirmations
            .process(&b, serde_json::from_str(&wire).unwrap())
            .unwrap(),
    );
    a.pairing_runtime.confirmations.process(&a, ack).unwrap();
    assert_ne!(peer(&b, "a").unwrap().key, base64_encode(&[42; 32]));
    let active = peer(&b, "a").unwrap().key;
    b.pairing_runtime
        .confirmations
        .process(&b, serde_json::from_str(&wire).unwrap())
        .unwrap();
    assert_eq!(peer(&b, "a").unwrap().key, active);
}

#[tokio::test]
async fn target_change_and_concurrent_pin_change_cannot_commit() {
    let (_ad, a) = fixture("a");
    let (_bd, b) = fixture("b");
    let a = a.runtime_state();
    let b = b.runtime_state();
    let (request, ticket) = begin(&a, &b);
    let response = b
        .pairing_runtime
        .confirmations
        .respond(&b, &request, true, device())
        .unwrap();
    let mut forged = response.clone();
    forged.signature = base64_encode(&[9; 64]);
    assert!(
        a.pairing_runtime
            .confirmations
            .response(&a, &forged, "")
            .is_err()
    );
    a.pairing_runtime
        .confirmations
        .response(&a, &response, "")
        .unwrap();
    let finish = verification(
        a.pairing_runtime
            .confirmations
            .confirm(&a, &ticket.generation)
            .unwrap(),
    );
    peer_store::save(
        &b.db,
        Facts {
            id: "a".into(),
            name: "Other".into(),
            ips: vec![],
            port: 2443,
            device_type: crate::features::chat::enums::DeviceType::Phone,
            key: base64_encode(&[42; 32]),
            public_key: base64_encode(&[9; 32]),
        },
    )
    .unwrap();
    assert!(b.pairing_runtime.confirmations.process(&b, finish).is_err());
    assert_eq!(peer(&b, "a").unwrap().key, base64_encode(&[42; 32]));
}

#[tokio::test]
async fn activation_holds_generation_lock_through_database_commit() {
    let (_ad, a) = fixture("a");
    let (_bd, b) = fixture("b");
    let a = a.runtime_state();
    let b = b.runtime_state();
    let (request, ticket) = begin(&a, &b);
    let response = b
        .pairing_runtime
        .confirmations
        .respond(&b, &request, true, device())
        .unwrap();
    a.pairing_runtime
        .confirmations
        .response(&a, &response, "")
        .unwrap();
    let finish = verification(
        a.pairing_runtime
            .confirmations
            .confirm(&a, &ticket.generation)
            .unwrap(),
    );
    let pairing = b.pairing_runtime.clone();
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = count.clone();
    b.db.with_conn(|c| {
        c.progress_handler(
            1,
            Some(move || {
                assert!(
                    pairing.commit.try_lock().is_err(),
                    "generation replacement must not interleave with activation"
                );
                observed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                false
            }),
        );
        Ok::<_, anyhow::Error>(())
    })
    .unwrap();
    let result = b.pairing_runtime.confirmations.process(&b, finish);
    b.db.with_conn(|c| {
        c.progress_handler(0, None::<fn() -> bool>);
        Ok::<_, anyhow::Error>(())
    })
    .unwrap();
    assert!(result.is_ok());
    assert!(count.load(std::sync::atomic::Ordering::Relaxed) > 0);
}
