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
    let prefs =
        plain_rs::prefs::Prefs::load(&plain_rs::prefs::default_path(directory.path())).unwrap();
    assert_eq!(
        prefs
            .get::<String>(plain_rs::prefs::pref_keys::PASSWORD)
            .unwrap()
            .as_deref(),
        Some("example-password")
    );
}
