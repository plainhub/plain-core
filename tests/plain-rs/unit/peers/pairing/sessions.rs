use super::*;
fn target() -> Target {
    Target {
        device_id: "peer".into(),
        device_name: "Fixture".into(),
        device_ip: "127.0.0.1".into(),
        device_port: 1,
    }
}
#[test]
fn old_timer_and_failure_cleanup_cannot_remove_replacement_session() {
    let sessions = Sessions::default();
    let old = sessions.start(target(), EcdhSession::generate());
    let next = sessions.start(target(), EcdhSession::generate());
    assert!(sessions.cancel("peer", Some(&old.generation)).is_none());
    assert!(sessions.expire("peer", &old.generation).is_none());
    assert!(sessions.expire("peer", &next.generation).is_none());
    sessions.0.lock().unwrap().get_mut("peer").unwrap().started = Instant::now() - RESPONSE_TIMEOUT;
    assert_eq!(
        sessions
            .expire("peer", &next.generation)
            .unwrap()
            .generation,
        next.generation
    );
    assert!(sessions.take("peer").is_none());
}
#[test]
fn concurrent_responses_consume_ephemeral_key_once() {
    let sessions = std::sync::Arc::new(Sessions::default());
    sessions.start(target(), EcdhSession::generate());
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let sessions = sessions.clone();
            std::thread::spawn(move || sessions.take("peer").is_some())
        })
        .collect();
    assert_eq!(
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .filter(|won| *won)
            .count(),
        1
    );
}

#[test]
fn batch_expiry_keeps_fresh_replacements_and_returns_each_expired_ticket_once() {
    let sessions = Sessions::default();
    let old = sessions.start(target(), EcdhSession::generate());
    sessions.0.lock().unwrap().get_mut("peer").unwrap().started = Instant::now() - RESPONSE_TIMEOUT;
    let fresh = sessions.start(target(), EcdhSession::generate());
    assert!(sessions.expire_all().is_empty());
    assert_eq!(sessions.tickets()[0].generation, fresh.generation);
    assert_ne!(old.generation, fresh.generation);
    sessions.0.lock().unwrap().get_mut("peer").unwrap().started = Instant::now() - RESPONSE_TIMEOUT;
    assert!(sessions.tickets().is_empty());
    assert_eq!(sessions.expire_all()[0].generation, fresh.generation);
    assert!(sessions.expire_all().is_empty());
}

#[test]
fn remaining_deadline_and_checked_take_follow_current_generation() {
    let sessions = Sessions::default();
    let old = sessions.start(target(), EcdhSession::generate());
    assert_eq!(sessions.states()[0].1, "STARTING");
    assert!(sessions.mark_sent("peer", &old.generation));
    assert_eq!(sessions.states()[0].1, "PAIRING");
    let remaining = sessions.remaining_ms("peer", &old.generation).unwrap();
    assert!(remaining > 89000 && remaining <= 90000);
    let next = sessions.start(target(), EcdhSession::generate());
    assert!(sessions.remaining_ms("peer", &old.generation).is_none());
    assert!(
        sessions
            .take_checked("peer", Some(&old.generation))
            .is_none()
    );
    sessions.0.lock().unwrap().get_mut("peer").unwrap().started = Instant::now() - RESPONSE_TIMEOUT;
    assert!(sessions.remaining_ms("peer", &next.generation).is_none());
    assert_eq!(
        sessions
            .take_checked("peer", Some(&next.generation))
            .unwrap()
            .ticket
            .generation,
        next.generation
    );
    assert!(
        sessions
            .take_checked("peer", Some(&next.generation))
            .is_none()
    );
}

fn incoming_request() -> crate::peers::pairing::protocol::PairingRequest {
    crate::peers::pairing::protocol::PairingRequest {
        from_id: "peer".into(),
        from_name: "Fixture".into(),
        port: 2443,
        device_type: "PHONE".into(),
        ecdh_public_key: String::new(),
        signature_public_key: "identity".into(),
        timestamp: crate::peers::pairing::now_ms(),
        ips: vec![],
        signature: "request".into(),
        aware_supported: false,
        from_ip: "127.0.0.1".into(),
    }
}

#[test]
fn incoming_approval_expires_at_response_deadline_and_replay_cannot_extend_it() {
    let sessions = Sessions::default();
    let mut request = incoming_request();
    assert!(sessions.receive(&request));
    assert!(sessions.incoming_target("peer", "request").is_some());
    sessions.1.lock().unwrap().get_mut("peer").unwrap().received =
        Instant::now() - RESPONSE_TIMEOUT;
    assert!(crate::peers::pairing::timestamp_ok(request.timestamp));
    assert!(sessions.incoming_target("peer", "request").is_none());
    assert!(sessions.take_incoming("peer", "request").is_none());
    request.from_ip = "127.0.0.2".into();
    assert!(!sessions.receive(&request));
    assert!(sessions.incoming_target("peer", "request").is_none());
    assert!(sessions.take_incoming("peer", "request").is_none());
    request.signature = "replacement".into();
    request.timestamp += 1;
    assert!(sessions.receive(&request));
    assert!(sessions.take_incoming("peer", "request").is_none());
    assert_eq!(
        sessions
            .take_incoming("peer", "replacement")
            .unwrap()
            .device_ip,
        "127.0.0.2"
    );
    assert!(sessions.take_incoming("peer", "replacement").is_none());
}

#[test]
fn incoming_approval_rechecks_signed_timestamp_when_consuming_request() {
    let sessions = Sessions::default();
    let request = incoming_request();
    assert!(sessions.receive(&request));
    sessions
        .1
        .lock()
        .unwrap()
        .get_mut("peer")
        .unwrap()
        .timestamp = i64::MIN;
    assert!(sessions.incoming_target("peer", "request").is_none());
    assert!(sessions.take_incoming("peer", "request").is_none());
}
