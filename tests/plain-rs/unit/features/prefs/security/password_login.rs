use super::*;

#[test]
fn pending_password_setup_survives_security_bootstrap() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let prefs = Prefs::load(&directory.path().join("prefs.json")).unwrap();
    prefs.set(PASSWORD_TYPE, PasswordType::Fixed).unwrap();
    bootstrap(&prefs).unwrap();
    assert!(password_credential(&prefs).unwrap().is_none());
}

#[test]
fn configured_password_survives_repeated_bootstrap_and_password_mode_selection() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let prefs = Prefs::load(&directory.path().join("prefs.json")).unwrap();
    bootstrap(&prefs).unwrap();
    let identity = prefs.get::<String>(CLIENT_ID).unwrap();
    set_password_login(&prefs, "owned-password").unwrap();
    let password = password_credential(&prefs).unwrap();
    prefs.set(PASSWORD_TYPE, PasswordType::None).unwrap();
    configure_password_login(&prefs).unwrap();
    bootstrap(&prefs).unwrap();
    assert!(!password_setup_required(&prefs).unwrap());
    assert_eq!(password_credential(&prefs).unwrap(), password);
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
fn unsupported_stored_credentials_are_not_migrated() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let prefs = Prefs::load(&directory.path().join("prefs.json")).unwrap();
    prefs.set(PASSWORD, "unsupported-record").unwrap();
    bootstrap(&prefs).unwrap();
    assert!(password_credential(&prefs).is_err());
    assert_eq!(get_entry(&prefs, PASSWORD).unwrap(), Some(json!("")));
    set_password_login(&prefs, "replacement-password").unwrap();
    assert!(opaque_public_key(&prefs).unwrap().is_some());
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
        assert!(password_credential(&prefs).unwrap().is_none());
    }
    set_password_login(&prefs, "fixture-password").unwrap();
    let digest = plain_libs::crypto::sha512_hex("fixture-password");
    let credential = password_credential(&prefs).unwrap().unwrap();
    assert!(!credential.setup.is_empty());
    assert!(!credential.record.is_empty());
    assert!(opaque_public_key(&prefs).unwrap().is_some());
    let stored = std::fs::read_to_string(directory.path().join("prefs.json")).unwrap();
    assert!(!stored.contains("fixture-password"));
    assert!(!stored.contains(&digest));
}

#[test]
fn password_replacement_rotates_opaque_identity_without_changing_device_identity() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let prefs = Prefs::load(&directory.path().join("prefs.json")).unwrap();
    bootstrap(&prefs).unwrap();
    let identity = prefs.get::<String>(SIGNATURE_KEY_PAIR).unwrap();
    set_password_login(&prefs, "first-password").unwrap();
    let first = opaque_public_key(&prefs).unwrap();
    set_password_login(&prefs, "second-password").unwrap();
    assert_ne!(opaque_public_key(&prefs).unwrap(), first);
    assert_eq!(prefs.get::<String>(SIGNATURE_KEY_PAIR).unwrap(), identity);
}

fn authenticate(prefs: &Prefs, password: &str) -> bool {
    let Ok(Some(credential)) = password_credential(prefs) else {
        return false;
    };
    let identity = plain_libs::utils::base64::base64_decode(
        &prefs.get::<String>(SIGNATURE_KEY_PAIR).unwrap().unwrap(),
    );
    let setup = plain_libs::utils::base64::base64_decode(&credential.setup);
    let record = plain_libs::utils::base64::base64_decode(&credential.record);
    let Ok(server_key) = plain_libs::auth::server::public_key(&setup) else {
        return false;
    };
    let client = plain_libs::auth::client::OpaqueLogin::start(password.as_bytes().to_vec()).unwrap();
    let context = plain_libs::auth::context("prefs-test", &identity[32..]);
    let Ok((server, response)) = plain_libs::auth::server::start(
        &setup,
        &record,
        &client.request(),
        &context,
        &identity[32..],
    ) else {
        return false;
    };
    let Ok(result) = client.finish(&response, "prefs-test", &identity[32..], &server_key) else {
        return false;
    };
    let Ok(session_key) = server.finish(&result.ke3) else {
        return false;
    };
    result.api_key.as_slice() == plain_libs::auth::api_key(&session_key, &context)
}

#[test]
fn registration_and_password_replacement_survive_reload() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let path = directory.path().join("prefs.json");
    let prefs = Prefs::load(&path).unwrap();
    set_password_login(&prefs, "first-password").unwrap();
    let prefs = Prefs::load(&path).unwrap();
    assert!(authenticate(&prefs, "first-password"));
    set_password_login(&prefs, "next-password").unwrap();
    let prefs = Prefs::load(&path).unwrap();
    assert!(!authenticate(&prefs, "first-password"));
    assert!(authenticate(&prefs, "next-password"));
}

#[test]
fn missing_or_replaced_setup_fails_closed_until_local_password_reset() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let path = directory.path().join("prefs.json");
    let prefs = Prefs::load(&path).unwrap();
    set_password_login(&prefs, "local-password").unwrap();
    let original = password_credential(&prefs).unwrap().unwrap();
    prefs
        .set(PASSWORD, json!({"record":original.record}))
        .unwrap();
    let prefs = Prefs::load(&path).unwrap();
    assert!(password_credential(&prefs).is_err());
    assert!(opaque_public_key(&prefs).is_err());
    assert!(!authenticate(&prefs, "local-password"));
    set_password_login(&prefs, "local-password").unwrap();
    let mut mixed = password_credential(&prefs).unwrap().unwrap();
    mixed.record = original.record;
    prefs.set(PASSWORD, mixed).unwrap();
    assert!(!authenticate(
        &Prefs::load(&path).unwrap(),
        "local-password"
    ));
    set_password_login(&prefs, "local-password").unwrap();
    assert!(authenticate(&Prefs::load(&path).unwrap(), "local-password"));
}

#[test]
fn failed_registration_save_preserves_both_old_setup_and_record() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let path = directory.path().join("prefs.json");
    let prefs = Prefs::load(&path).unwrap();
    set_password_login(&prefs, "kept-password").unwrap();
    let credential = password_credential(&prefs).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    std::fs::create_dir(path.with_extension("json.tmp")).unwrap();
    assert!(set_password_login(&prefs, "interrupted-password").is_err());
    assert_eq!(password_credential(&prefs).unwrap(), credential);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let reloaded = Prefs::load(&path).unwrap();
    assert!(authenticate(&reloaded, "kept-password"));
    assert!(!authenticate(&reloaded, "interrupted-password"));
}
