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
fn configured_password_survives_repeated_bootstrap_and_password_mode_selection() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let prefs = Prefs::load(&directory.path().join("prefs.json")).unwrap();
    bootstrap(&prefs).unwrap();
    let identity = prefs.get::<String>(CLIENT_ID).unwrap();
    set_password_login(&prefs, "owned-password").unwrap();
    let password = password_hash(&prefs).unwrap();
    prefs.set(PASSWORD_TYPE, PasswordType::None).unwrap();
    configure_password_login(&prefs).unwrap();
    bootstrap(&prefs).unwrap();
    assert!(!password_setup_required(&prefs).unwrap());
    assert_eq!(password_hash(&prefs).unwrap(), password);
    assert_eq!(prefs.get::<String>(CLIENT_ID).unwrap(), identity);
}

#[test]
fn password_login_disables_approval_without_replacing_credentials_or_other_host_defaults() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let prefs = Prefs::load(&directory.path().join("prefs.json")).unwrap();
    bootstrap(&prefs).unwrap();
    assert_eq!(password_type(&prefs).unwrap(), PasswordType::None);
    assert!(!password_setup_required(&prefs).unwrap());
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
fn password_setting_validates_input_and_never_stores_plaintext_or_client_digest() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let prefs = Prefs::load(&directory.path().join("prefs.json")).unwrap();
    for invalid in ["", "  ", &"x".repeat(1025)] {
        assert!(matches!(
            set_password_login(&prefs, invalid),
            Err(PasswordError::InvalidPassword)
        ));
        assert!(password_hash(&prefs).unwrap().is_none());
    }
    set_password_login(&prefs, "fixture-password").unwrap();
    let digest = plain_libs::crypto::sha512_hex("fixture-password");
    assert!(
        password_hash(&prefs)
            .unwrap()
            .unwrap()
            .verify(digest.as_bytes())
    );
    let stored = std::fs::read_to_string(directory.path().join("prefs.json")).unwrap();
    assert!(!stored.contains("fixture-password"));
    assert!(!stored.contains(&digest));
}
