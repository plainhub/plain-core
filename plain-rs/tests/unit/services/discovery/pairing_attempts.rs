use super::*;

#[test]
fn continuation_cannot_retarget_or_reuse_a_terminal_attempt() {
    let attempts = Attempts::default();
    let (first, _, _) = attempts
        .begin(
            None,
            Some("peer".into()),
            Some("example.test:443".into()),
            Mode::Manage,
        )
        .unwrap();
    {
        let mut entries = attempts.0.lock().unwrap();
        entries.get_mut(&first.generation).unwrap().state.phase = "PASSWORD_REQUIRED".into();
    }
    for (id, host, mode) in [
        (Some("other"), None, None),
        (None, Some("other.test"), None),
        (None, None, Some(Mode::Chat)),
    ] {
        assert!(
            attempts
                .submit(None, &first.generation, id, host, mode, "secret".into())
                .is_err()
        );
    }
    let (second, _, _) = attempts
        .begin(None, Some("peer".into()), None, Mode::Manage)
        .unwrap();
    assert_eq!(attempts.get(&first.generation).unwrap().phase, "CANCELLED");
    assert!(
        attempts
            .submit(None, &first.generation, None, None, None, "secret".into())
            .is_err()
    );
    assert_eq!(attempts.get(&second.generation).unwrap().phase, "STARTING");
    let encoded = serde_json::to_value(attempts.states()).unwrap().to_string();
    assert!(!encoded.contains("secret"));
    assert!(!encoded.contains("example.test"));
}

#[test]
fn attempt_retention_is_bounded_and_preserves_active_work() {
    let attempts = Attempts::default();
    for index in 0..128 {
        attempts
            .begin(None, Some(format!("peer-{index}")), None, Mode::Chat)
            .unwrap();
    }
    assert!(
        attempts
            .begin(None, Some("overflow".into()), None, Mode::Chat)
            .is_err()
    );
    let generation = attempts.states()[0].generation.clone();
    attempts
        .0
        .lock()
        .unwrap()
        .get_mut(&generation)
        .unwrap()
        .state
        .phase = "SUCCEEDED".into();
    attempts
        .begin(None, Some("replacement".into()), None, Mode::Manage)
        .unwrap();
    assert_eq!(attempts.states().len(), 128);
    assert!(attempts.get(&generation).is_none());
}

#[test]
fn authenticating_an_older_manual_address_cannot_replace_a_newer_identity_attempt() {
    let attempts = Attempts::default();
    let (older, _, _) = attempts
        .begin(None, None, Some("old.test".into()), Mode::Manage)
        .unwrap();
    let (newer, _, _) = attempts
        .begin(None, Some("peer".into()), None, Mode::Manage)
        .unwrap();
    assert!(
        attempts
            .claim_identity(None, &older.generation, "peer")
            .is_err()
    );
    assert!(
        attempts
            .claim_identity(None, &newer.generation, "peer")
            .is_ok()
    );
}
