use super::*;

#[test]
fn commands_target_the_current_peer_and_confirmation_is_consumed() {
    let attempts = Attempts::default();
    let old = attempts.begin("peer", Mode::Manage).unwrap();
    let current = attempts.begin("peer", Mode::Chat).unwrap();
    let other = attempts.begin("other", Mode::Manage).unwrap();
    assert!(*old.borrow());
    assert!(!*current.borrow());
    assert!(!*other.borrow());
    assert_eq!(attempts.mode("peer"), Some(Mode::Chat));
    assert!(attempts.confirm("peer").is_err());
    let accepted = attempts.request_confirmation("peer").unwrap();
    attempts.confirm("peer").unwrap();
    assert!(*accepted.borrow());
    assert!(!attempts.awaiting_confirmation("peer"));
    assert!(attempts.confirm("peer").is_err());
}

#[test]
fn active_pairing_capacity_preserves_existing_peers() {
    let attempts = Attempts::default();
    for index in 0..128 {
        attempts
            .begin(&format!("peer-{index}"), Mode::Chat)
            .unwrap();
    }
    assert!(attempts.begin("overflow", Mode::Chat).is_err());
    assert!(attempts.begin("peer-0", Mode::Manage).is_ok());
    assert_eq!(attempts.mode("peer-0"), Some(Mode::Manage));
}
