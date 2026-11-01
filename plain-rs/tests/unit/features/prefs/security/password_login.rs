use super::*;

#[test]
fn pending_password_setup_survives_security_bootstrap() {
    let directory = tempfile::tempdir().unwrap();
    let prefs = Prefs::load(&directory.path().join("prefs.json")).unwrap();
    prefs.set(PASSWORD_TYPE, PasswordType::Fixed).unwrap();
    prefs.set(PASSWORD, "").unwrap();
    bootstrap(&prefs).unwrap();
    assert_eq!(prefs.get::<String>(PASSWORD).unwrap().as_deref(), Some(""));
}

#[test]
fn password_login_starts_with_setup_then_preserves_the_configured_credential() {
    let directory = tempfile::tempdir().unwrap();
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
    assert_eq!(
        prefs.get::<String>(PASSWORD).unwrap().as_deref(),
        Some("owned-password")
    );
    assert_eq!(prefs.get::<String>(CLIENT_ID).unwrap(), identity);
}

#[test]
fn concurrent_setup_only_commits_one_password() {
    let directory = tempfile::tempdir().unwrap();
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
    assert_eq!(
        prefs.get::<String>(PASSWORD).unwrap().as_deref(),
        Some(expected)
    );
}

#[test]
fn fixed_credentials_are_not_replaced_and_mobile_defaults_still_use_owner_approval() {
    let directory = tempfile::tempdir().unwrap();
    let prefs = Prefs::load(&directory.path().join("prefs.json")).unwrap();
    bootstrap(&prefs).unwrap();
    assert_eq!(password_type(&prefs).unwrap(), PasswordType::None);
    assert!(!password_setup_required(&prefs).unwrap());
    assert!(!setup_password(&prefs, "unauthorized").unwrap());
    assert!(prefs.get_or(LOGIN_REQUIRE_APPROVAL, true));
    set_password_login(&prefs, "configured-password").unwrap();
    prefs.set(LOGIN_REQUIRE_APPROVAL, true).unwrap();
    let before = prefs.entries();
    configure_password_login(&prefs).unwrap();
    assert_eq!(prefs.entries(), before);
}
