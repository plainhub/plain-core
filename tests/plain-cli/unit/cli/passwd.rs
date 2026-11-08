use super::*;

#[test]
fn password_command_writes_shared_system_credentials() {
    let directory = plain_libs::temporary::tempdir().unwrap();
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
    assert!(
        plain_rs::features::prefs::security::password_hash(&prefs)
            .unwrap()
            .unwrap()
            .verify(plain_libs::crypto::sha512_hex("example-password").as_bytes())
    );
    let stored =
        std::fs::read_to_string(plain_rs::features::prefs::default_path(directory.path())).unwrap();
    assert!(!stored.contains("example-password"));
    assert_eq!(
        plain_rs::features::prefs::security::password_type(&prefs).unwrap(),
        plain_rs::features::prefs::security::PasswordType::Fixed
    );
    assert!(!prefs.get_or("login_require_approval", true));
}
