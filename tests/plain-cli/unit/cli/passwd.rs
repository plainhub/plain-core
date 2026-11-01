use super::*;

#[test]
fn password_command_writes_shared_system_credentials() {
    let directory = tempfile::tempdir().unwrap();
    set_password(
        directory.path(),
        &directory.path().join("cache"),
        "example-password",
    )
    .unwrap();
    let prefs = plain_rs::features::prefs::Prefs::load(&plain_rs::features::prefs::default_path(
        directory.path(),
    ))
    .unwrap();
    assert_eq!(
        prefs
            .get::<String>(plain_rs::features::prefs::pref_keys::PASSWORD)
            .unwrap()
            .as_deref(),
        Some("example-password")
    );
    assert_eq!(
        plain_rs::features::prefs::security::password_type(&prefs).unwrap(),
        plain_rs::features::prefs::security::PasswordType::Fixed
    );
    assert!(!prefs.get_or("login_require_approval", true));
}
