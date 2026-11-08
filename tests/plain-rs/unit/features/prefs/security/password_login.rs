use super::*;

#[test]
fn pending_password_setup_survives_security_bootstrap() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let prefs = Prefs::load(&directory.path().join("prefs.json")).unwrap();
    prefs.set(PASSWORD_TYPE, PasswordType::Fixed).unwrap();
    prefs.set(PASSWORD, "").unwrap();
    bootstrap(&prefs).unwrap();
    assert!(password_hash(&prefs).unwrap().is_none());
}

#[test]
fn password_login_starts_with_setup_then_preserves_the_configured_credential() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let prefs = Prefs::load(&directory.path().join("prefs.json")).unwrap();
    bootstrap(&prefs).unwrap();
    let identity = prefs.get::<String>(CLIENT_ID).unwrap();
    configure_password_login(&prefs).unwrap();
    assert!(password_setup_required(&prefs).unwrap());
    assert!(!prefs.get_or(LOGIN_REQUIRE_APPROVAL, true));
    assert!(matches!(
        setup_password(&prefs, "  "),
        Err(PasswordSetupError::InvalidPassword)
    ));
    assert!(setup_password(&prefs, "owned-password").unwrap());
    assert!(!setup_password(&prefs, "replacement").unwrap());
    configure_password_login(&prefs).unwrap();
    bootstrap(&prefs).unwrap();
    assert!(!password_setup_required(&prefs).unwrap());
    assert!(
        password_hash(&prefs)
            .unwrap()
            .unwrap()
            .verify(plain_libs::crypto::sha512_hex("owned-password").as_bytes())
    );
    let stored = std::fs::read_to_string(directory.path().join("prefs.json")).unwrap();
    assert!(!stored.contains("owned-password"));
    assert!(!stored.contains(&plain_libs::crypto::sha512_hex("owned-password")));
    assert_eq!(prefs.get::<String>(CLIENT_ID).unwrap(), identity);
}

#[test]
fn concurrent_setup_only_commits_one_password() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let prefs = Prefs::load(&directory.path().join("prefs.json")).unwrap();
    bootstrap(&prefs).unwrap();
    configure_password_login(&prefs).unwrap();
    let results = std::thread::scope(|scope| {
        let first = scope.spawn(|| setup_password(&prefs, "first-password").unwrap());
        let second = scope.spawn(|| setup_password(&prefs, "second-password").unwrap());
        (first.join().unwrap(), second.join().unwrap())
    });
    assert_ne!(results.0, results.1);
    let expected = if results.0 {
        "first-password"
    } else {
        "second-password"
    };
    assert!(
        password_hash(&prefs)
            .unwrap()
            .unwrap()
            .verify(plain_libs::crypto::sha512_hex(expected).as_bytes())
    );
}

#[test]
fn password_login_disables_approval_without_replacing_credentials_or_other_host_defaults() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let prefs = Prefs::load(&directory.path().join("prefs.json")).unwrap();
    bootstrap(&prefs).unwrap();
    assert_eq!(password_type(&prefs).unwrap(), PasswordType::None);
    assert!(!password_setup_required(&prefs).unwrap());
    assert!(!setup_password(&prefs, "unauthorized").unwrap());
    assert!(prefs.get_or(LOGIN_REQUIRE_APPROVAL, true));
    set_password_login(&prefs, "configured-password").unwrap();
    prefs.set(LOGIN_REQUIRE_APPROVAL, true).unwrap();
    let mut expected = prefs.entries();
    expected
        .iter_mut()
        .find(|(key, _)| key == LOGIN_REQUIRE_APPROVAL)
        .unwrap()
        .1 = json!(false);
    configure_password_login(&prefs).unwrap();
    assert_eq!(prefs.entries(), expected);
    bootstrap(&prefs).unwrap();
    assert!(!prefs.get_or(LOGIN_REQUIRE_APPROVAL, true));
}

#[test]
fn startup_replaces_existing_plaintext_without_rehashing_a_stored_verifier() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let prefs = Prefs::load(&directory.path().join("prefs.json")).unwrap();
    prefs.set(PASSWORD, "existing-fixture-password").unwrap();
    prefs.set(PASSWORD_TYPE, PasswordType::Fixed).unwrap();
    bootstrap(&prefs).unwrap();
    let stored = password_hash(&prefs).unwrap().unwrap();
    assert!(stored.verify(plain_libs::crypto::sha512_hex("existing-fixture-password").as_bytes()));
    bootstrap(&prefs).unwrap();
    assert_eq!(password_hash(&prefs).unwrap(), Some(stored));
    assert_eq!(get_entry(&prefs, PASSWORD).unwrap(), Some(json!("")));
    let bytes = std::fs::read_to_string(directory.path().join("prefs.json")).unwrap();
    assert!(!bytes.contains("existing-fixture-password"));
}

#[test]
fn setup_accepts_only_client_digests_and_never_stores_the_digest() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let prefs = Prefs::load(&directory.path().join("prefs.json")).unwrap();
    configure_password_login(&prefs).unwrap();
    for invalid in ["", "plaintext-password", "ABCDEF"] {
        assert!(setup_password_hash(&prefs, invalid).is_err());
        assert!(password_setup_required(&prefs).unwrap());
    }
    let digest = plain_libs::crypto::sha512_hex("fixture-password");
    assert!(setup_password_hash(&prefs, &digest).unwrap());
    assert!(
        password_hash(&prefs)
            .unwrap()
            .unwrap()
            .verify(digest.as_bytes())
    );
    assert!(
        !std::fs::read_to_string(directory.path().join("prefs.json"))
            .unwrap()
            .contains(&digest)
    );
}
