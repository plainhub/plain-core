use super::*;
use plain_rs::features::prefs::{Prefs, default_path, security};

fn paths(root: &std::path::Path) -> AppPaths {
    AppPaths {
        data_dir: root.join("data"),
        cache_dir: root.join("cache"),
        tls_cert: root.join("tls.pem"),
        tls_key: root.join("tls.key"),
    }
}

#[test]
fn installation_sets_a_password_and_reinstallation_never_prompts_or_overwrites_it() {
    let root = plain_libs::temporary::tempdir().unwrap();
    let paths = paths(root.path());
    prepare_password(&paths, || Ok("installed password".into())).unwrap();
    let prefs = Prefs::load(&default_path(&paths.data_dir)).unwrap();
    let hash = security::password_hash(&prefs).unwrap().unwrap();
    assert!(hash.verify(plain_libs::crypto::sha512_hex("installed password").as_bytes()));
    assert_eq!(
        security::password_type(&prefs).unwrap(),
        security::PasswordType::Fixed
    );
    assert!(!prefs.get_or("login_require_approval", true));
    let before = std::fs::read(default_path(&paths.data_dir)).unwrap();
    prepare_password(&paths, || panic!("Upgrade must not prompt")).unwrap();
    assert_eq!(
        std::fs::read(default_path(&paths.data_dir)).unwrap(),
        before
    );
    assert!(
        !String::from_utf8(before)
            .unwrap()
            .contains("installed password")
    );
}

#[test]
fn cancelled_or_invalid_installation_never_creates_a_password() {
    for value in [None, Some(""), Some("  ")] {
        let root = plain_libs::temporary::tempdir().unwrap();
        let paths = paths(root.path());
        assert!(
            prepare_password(&paths, || match value {
                Some(value) => Ok(value.into()),
                None => anyhow::bail!("No interactive terminal"),
            })
            .is_err()
        );
        let prefs = Prefs::load(&default_path(&paths.data_dir)).unwrap();
        assert!(security::password_hash(&prefs).unwrap().is_none());
    }
}

#[test]
fn installation_reuses_shared_bootstrap_to_preserve_an_existing_plaintext_password() {
    let root = plain_libs::temporary::tempdir().unwrap();
    let paths = paths(root.path());
    std::fs::create_dir_all(&paths.data_dir).unwrap();
    let prefs = Prefs::load(&default_path(&paths.data_dir)).unwrap();
    prefs.set("password", "existing password").unwrap();
    prefs.set("password_type", "FIXED").unwrap();
    prepare_password(&paths, || panic!("Existing password must not prompt")).unwrap();
    let prefs = Prefs::load(&default_path(&paths.data_dir)).unwrap();
    assert!(
        security::password_hash(&prefs)
            .unwrap()
            .unwrap()
            .verify(plain_libs::crypto::sha512_hex("existing password").as_bytes())
    );
    assert!(
        !std::fs::read_to_string(default_path(&paths.data_dir))
            .unwrap()
            .contains("existing password")
    );
}

#[test]
fn invalid_stored_credentials_fail_without_prompting_or_overwriting_them() {
    let root = plain_libs::temporary::tempdir().unwrap();
    let paths = paths(root.path());
    std::fs::create_dir_all(&paths.data_dir).unwrap();
    let prefs = Prefs::load(&default_path(&paths.data_dir)).unwrap();
    prefs.set("password", 42).unwrap();
    assert!(prepare_password(&paths, || panic!("Invalid credentials must not prompt")).is_err());
    assert_eq!(
        Prefs::load(&default_path(&paths.data_dir))
            .unwrap()
            .get::<u32>("password")
            .unwrap(),
        Some(42)
    );
}
