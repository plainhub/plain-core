use crate::features::access::AccessFeature;
use super::*;
fn fixture() -> (plain_libs::temporary::TempDir, Prefs) {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs = Prefs::load(&dir.path().join("prefs.json")).unwrap();
    (dir, prefs)
}
#[test]
fn bootstrap_is_stable_and_snapshot_never_contains_private_secrets() {
    let (_dir, prefs) = fixture();
    bootstrap(&prefs).unwrap();
    let first = serde_json::to_value(snapshot(&prefs).unwrap()).unwrap();
    bootstrap(&prefs).unwrap();
    assert_eq!(
        first,
        serde_json::to_value(snapshot(&prefs).unwrap()).unwrap()
    );
    assert_eq!(first["passwordType"], json!(PasswordType::None));
    assert_eq!(first["loginRequireApproval"], true);
    assert!(!first["clientId"].as_str().unwrap().is_empty());
    assert_eq!(signing_keypair(&prefs).unwrap().len(), 64);
    assert!(first.get("masterSecret").is_none());
    assert!(first.get("signatureKeyPair").is_none());
}
#[test]
fn parallel_permission_updates_do_not_lose_changes() {
    let (_dir, prefs) = fixture();
    bootstrap(&prefs).unwrap();
    std::thread::scope(|scope| {
        for name in [AccessFeature::Images, AccessFeature::Adb, AccessFeature::Sms, AccessFeature::Clipboard] {
            let prefs = &prefs;
            scope.spawn(move || {
                Command::SetApiPermission {
                    permission: name,
                    enabled: true,
                }
                .apply(prefs)
                .unwrap();
            });
        }
    });
    assert_eq!(snapshot(&prefs).unwrap().api_permissions.len(), 4);
    assert!(serde_json::from_value::<Command>(serde_json::json!({
        "action":"setApiPermission", "permission":"UNKNOWN", "enabled":true,
    })).is_err());
}

#[test]
fn runtime_preferences_use_shared_defaults_and_persist_only_system_values() {
    let (dir, prefs) = fixture();
    bootstrap(&prefs).unwrap();
    assert_eq!(dev_token(&prefs).unwrap(), "");
    assert_eq!(log_level(&prefs).unwrap(), log::LevelFilter::Info);
    assert_eq!(
        prefs.get::<String>(LOG_LEVEL).unwrap().as_deref(),
        Some("info")
    );
    set_entry(&prefs, DEV_TOKEN, serde_json::json!("development")).unwrap();
    set_entry(&prefs, LOG_LEVEL, serde_json::json!("warn")).unwrap();
    bootstrap(&prefs).unwrap();
    let reloaded = Prefs::load(&dir.path().join("prefs.json")).unwrap();
    assert_eq!(dev_token(&reloaded).unwrap(), "development");
    assert_eq!(log_level(&reloaded).unwrap(), log::LevelFilter::Warn);
    assert!(!reloaded.entries().is_empty());
    for value in [
        serde_json::json!("invalid"),
        serde_json::json!(1),
        serde_json::json!(null),
    ] {
        assert!(set_entry(&prefs, LOG_LEVEL, value).is_err());
    }
    assert!(set_entry(&prefs, DEV_TOKEN, serde_json::json!(true)).is_err());
    assert_eq!(log_level(&prefs).unwrap(), log::LevelFilter::Warn);
    remove_entry(&prefs, LOG_LEVEL).unwrap();
    assert_eq!(log_level(&prefs).unwrap(), log::LevelFilter::Info);
}

#[test]
fn key_value_settings_validate_protected_fields_and_patch_permissions_atomically() {
    let dir = plain_libs::temporary::tempdir().unwrap();
    let prefs = Prefs::load(&dir.path().join("prefs.json")).unwrap();
    bootstrap(&prefs).unwrap();
    let before = prefs.entries();
    for (key, value) in [
        (PASSWORD_TYPE, serde_json::json!(4)),
        (LOGIN_REQUIRE_APPROVAL, serde_json::json!("false")),
        (SIGNATURE_KEY_PAIR, serde_json::json!("client-supplied")),
        (MDNS_HOSTNAME, serde_json::json!("invalid.example")),
    ] {
        assert!(set_entry(&prefs, key, value).is_err());
        assert_eq!(prefs.entries(), before);
    }
    set_entry(&prefs, PASSWORD, serde_json::json!("Synthetic password")).unwrap();
    set_entry(&prefs, API_PERMISSIONS, serde_json::json!(["SMS", "AUDIO"])).unwrap();
    set_entry(
        &prefs,
        API_PERMISSIONS,
        serde_json::json!({"SMS":false,"CONTACTS":true}),
    )
    .unwrap();
    assert_eq!(
        snapshot(&prefs).unwrap().api_permissions,
        [AccessFeature::Audio, AccessFeature::Contacts]
    );
    let before = prefs.entries();
    assert!(
        set_entry(
            &prefs,
            API_PERMISSIONS,
            serde_json::json!({"SMS":true,"UNKNOWN":false})
        )
        .is_err()
    );
    assert_eq!(prefs.entries(), before);
}

#[test]
fn bootstrap_generates_mdns_name_once_for_new_or_empty_preferences() {
    for empty_value in [false, true] {
        let (_dir, prefs) = fixture();
        if empty_value {
            prefs.set(MDNS_HOSTNAME, "").unwrap();
        }
        bootstrap(&prefs).unwrap();
        let hostname = snapshot(&prefs).unwrap().mdns_hostname;
        let label = hostname.strip_suffix(".local").unwrap();
        assert_eq!(label.len(), 2);
        assert!(
            label
                .bytes()
                .all(|byte| b"abcdefghjkmnpqrstuwxyz".contains(&byte))
        );
        assert_eq!(
            crate::features::prefs::ensure_mdns_hostname(&prefs),
            hostname
        );
        let reloaded = Prefs::load(prefs.path()).unwrap();
        bootstrap(&reloaded).unwrap();
        assert_eq!(snapshot(&reloaded).unwrap().mdns_hostname, hostname);
    }
}

#[test]
fn bootstrap_preserves_the_saved_mdns_name() {
    let (_dir, prefs) = fixture();
    prefs.set(MDNS_HOSTNAME, "custom-device.local").unwrap();
    bootstrap(&prefs).unwrap();
    assert_eq!(
        snapshot(&prefs).unwrap().mdns_hostname,
        "custom-device.local"
    );
}

#[test]
fn keyed_security_reads_preserve_snapshot_defaults_without_reading_unrelated_keys() {
    let prefs = Prefs::in_memory();
    assert_eq!(
        get_entry(&prefs, PASSWORD_TYPE).unwrap(),
        Some(json!("NONE"))
    );
    assert_eq!(
        get_entry(&prefs, LOGIN_REQUIRE_APPROVAL).unwrap(),
        Some(json!(true))
    );
    assert_eq!(get_entry(&prefs, API_PERMISSIONS).unwrap(), Some(json!([])));
    assert_eq!(
        get_entry(&prefs, AUTO_CHECK_UPDATE).unwrap(),
        Some(json!(true))
    );
    assert_eq!(get_entry(&prefs, SKIP_VERSION).unwrap(), Some(json!("")));
    bootstrap(&prefs).unwrap();
    let state = snapshot(&prefs).unwrap();
    assert_eq!(
        get_entry(&prefs, CLIENT_ID).unwrap(),
        Some(json!(state.client_id))
    );
    assert_eq!(
        get_entry(&prefs, SIGNATURE_PUBLIC_KEY).unwrap(),
        Some(json!(state.signature_public_key))
    );
    prefs.set(SIGNATURE_KEY_PAIR, "invalid identity").unwrap();
    prefs.set(PASSWORD_TYPE, "invalid type").unwrap();
    prefs.set(API_PERMISSIONS, vec!["IMAGES"]).unwrap();
    assert_eq!(
        get_entry(&prefs, API_PERMISSIONS).unwrap(),
        Some(json!(["IMAGES"]))
    );
    assert_eq!(
        get_entry(&prefs, PASSWORD).unwrap(),
        Some(json!(state.password))
    );
    assert!(get_entry(&prefs, SIGNATURE_PUBLIC_KEY).is_err());
    assert!(get_entry(&prefs, PASSWORD_TYPE).is_err());
    for key in [MASTER_SECRET, SIGNATURE_KEY_PAIR, "unknown"] {
        assert_eq!(get_entry(&prefs, key).unwrap(), None, "{key}");
    }
}

#[test]
fn manage_files_patch_changes_all_five_features_and_preserves_other_access() {
    let (_directory, prefs) = fixture();
    set_entry(&prefs, API_PERMISSIONS, serde_json::json!(["CONTACTS"])).unwrap();
    for enabled in [true, false] {
        let patch: std::collections::BTreeMap<_, _> = AccessFeature::FILE_FEATURES.into_iter().map(|feature| (feature, enabled)).collect();
        set_entry(&prefs, API_PERMISSIONS, serde_json::to_value(patch).unwrap()).unwrap();
        assert!(crate::features::services::device::access::is_enabled(&prefs, &[AccessFeature::Contacts]));
        for feature in AccessFeature::FILE_FEATURES {
            assert_eq!(crate::features::services::device::access::is_enabled(&prefs, &[feature]), enabled);
        }
    }
}

#[test]
fn startup_never_generates_or_accepts_a_global_resource_token_preference() {
    let (_dir, prefs) = fixture();
    for _ in 0..2 {
        bootstrap(&prefs).unwrap();
        crate::features::prefs::identity::ensure_identity(&prefs);
        crate::features::prefs::identity::ensure_mdns_hostname(&prefs);
        for key in ["url_token", "urlToken"] {
            assert!(prefs.get::<String>(key).unwrap().is_none());
            assert!(prefs.set(key, "synthetic-retired-key").is_err());
            assert!(prefs.get::<String>(key).unwrap().is_none());
            assert!(
                Prefs::load(prefs.path())
                    .unwrap()
                    .get::<String>(key)
                    .unwrap()
                    .is_none()
            );
        }
    }
}
