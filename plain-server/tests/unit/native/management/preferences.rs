use crate::assembly::server::ContentServer;

#[tokio::test]
async fn startup_loads_published_integer_enums_and_returns_names_to_the_app() {
    use plain_rs::prefs::{Prefs, pref_keys as keys};

    let directory = tempfile::tempdir().unwrap();
    let path = plain_rs::prefs::default_path(directory.path());
    let prefs = Prefs::load(&path).unwrap();
    let expected = [
        (keys::DARK_THEME, "OFF"),
        (keys::AUDIO_PLAY_MODE, "SHUFFLE"),
        (keys::AUDIO_SORT_BY, "SIZE_ASC"),
        (keys::VIDEO_SORT_BY, "SIZE_ASC"),
        (keys::IMAGE_SORT_BY, "SIZE_ASC"),
        (keys::DOC_SORT_BY, "SIZE_ASC"),
        (keys::FILE_SORT_BY, "SIZE_ASC"),
        (keys::PACKAGE_SORT_BY, "SIZE_ASC"),
    ];
    for (key, _) in expected {
        prefs.set(key, 2).unwrap();
    }
    let core = ContentServer::open(directory.path()).unwrap();
    let values = core.native_management().unwrap().preferences().unwrap();
    let stored = Prefs::load(&path).unwrap();
    for (key, name) in expected {
        assert_eq!(values[key], serde_json::json!(name));
        assert_eq!(stored.get::<u64>(key).unwrap(), Some(2));
    }
    core.stop_core().await;
}

#[tokio::test]
async fn native_resets_keep_identity_and_publish_once_per_change() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let management = core.native_management().unwrap();
    let before = plain_rs::prefs::security::snapshot(&state.prefs).unwrap();
    let mut events = state.events.subscribe();
    assert!(management.reset_password().unwrap());
    assert!(management.reset_url_token().unwrap());
    assert!(management.reset_adb_token().unwrap());
    let after = plain_rs::prefs::security::snapshot(&state.prefs).unwrap();
    assert_ne!(before.password, after.password);
    assert_ne!(before.url_token, after.url_token);
    assert_ne!(before.adb_token, after.adb_token);
    assert_eq!(before.client_id, after.client_id);
    assert_eq!(before.signature_public_key, after.signature_public_key);
    assert_eq!(after.password.len(), 6);
    assert_eq!(after.adb_token.len(), 32);
    assert_eq!(
        plain_rs::utils::base64::base64_decode_checked(&after.url_token)
            .unwrap()
            .len(),
        32
    );
    for _ in 0..3 {
        assert_eq!(events.try_recv().unwrap().event_type, "PREFS_UPDATED");
    }
    assert!(events.try_recv().is_err());
    core.stop_core().await;
}

#[tokio::test]
async fn native_notification_actions_validate_and_preserve_change_events() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let management = core.native_management().unwrap();
    let mut events = state.events.subscribe();
    assert!(
        management
            .set_notification_filter_mode("allowlist".into())
            .await
            .unwrap()
    );
    assert_eq!(preference_updates(&mut events), 1);
    assert!(
        !management
            .set_notification_filter_mode("allowlist".into())
            .await
            .unwrap()
    );
    assert!(
        management
            .set_notification_filter_mode("invalid".into())
            .await
            .is_err()
    );
    assert_eq!(preference_updates(&mut events), 0);
    management
        .toggle_notification_app("fixture.app".into())
        .await
        .unwrap();
    assert_eq!(preference_updates(&mut events), 1);
    assert!(management.clear_notification_filter_apps().await.unwrap());
    assert_eq!(preference_updates(&mut events), 1);
    assert!(!management.clear_notification_filter_apps().await.unwrap());
    assert_eq!(preference_updates(&mut events), 0);
    core.stop_core().await;
}

#[tokio::test]
async fn native_scan_history_preserves_order_and_scoped_deletion() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let management = core.native_management().unwrap();
    assert!(management.scan_history().unwrap().is_empty());
    for text in ["duplicate", "neighbor", "duplicate", "quote'\nvalue"] {
        assert!(management.add_scan_history(text.into()).unwrap());
    }
    assert_eq!(
        management.scan_history().unwrap(),
        ["quote'\nvalue", "duplicate", "neighbor", "duplicate"]
    );
    assert!(management.remove_scan_history("duplicate".into()).unwrap());
    assert_eq!(
        management.scan_history().unwrap(),
        ["quote'\nvalue", "neighbor"]
    );
    core.stop_core().await;
}

fn preference_updates(events: &mut tokio::sync::broadcast::Receiver<plain_rs::events::WsEvent>) -> usize {
    std::iter::from_fn(|| events.try_recv().ok())
        .filter(|event| event.event_type == "PREFS_UPDATED")
        .count()
}

#[tokio::test]
async fn native_preference_writes_share_validation_and_commit_notifications() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let management = core.native_management().unwrap();
    let mut events = state.events.subscribe();
    assert!(
        management
            .set_pref("http_port", serde_json::json!(8181))
            .await
            .unwrap()
    );
    assert_eq!(preference_updates(&mut events), 1);
    assert_eq!(
        management.preferences().unwrap()["http_port"],
        8181
    );
    assert!(
        !management
            .set_pref("http_port", serde_json::json!(8181))
            .await
            .unwrap()
    );
    assert_eq!(preference_updates(&mut events), 0);
    let before = state.prefs.entries_sorted();
    for (key, value) in [
        ("http_port", serde_json::json!(70000)),
        ("url_token", serde_json::json!("invalid")),
        ("unknown_pref", serde_json::json!(true)),
        ("ai_image_search_model", serde_json::json!("unknown")),
    ] {
        assert!(management.set_pref(key, value).await.is_err());
    }
    assert_eq!(state.prefs.entries_sorted(), before);
    assert_eq!(preference_updates(&mut events), 0);
    assert!(
        management
            .set_pref("login_require_approval", serde_json::json!(false))
            .await
            .unwrap()
    );
    assert_eq!(preference_updates(&mut events), 1);
    assert_eq!(
        management.preferences().unwrap()["login_require_approval"],
        false
    );
    core.stop_core().await;
    assert!(
        management
            .set_pref("http_port", serde_json::json!(8282))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn keyed_native_reads_observe_committed_writes_and_reject_private_or_unknown_keys() {
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let management = core.native_management().unwrap();
    assert_eq!(management.get_pref("http_port").unwrap(), serde_json::json!(8080));
    assert!(management.set_pref("http_port", serde_json::json!(8282)).await.unwrap());
    assert_eq!(management.get_pref("http_port").unwrap(), serde_json::json!(8282));
    let keys = vec!["http_port".into(), "client_id".into()];
    let selected = management.get_prefs(&keys).unwrap();
    assert_eq!(selected.len(), 2);
    assert_eq!(selected["http_port"], serde_json::json!(8282));
    assert_eq!(selected["client_id"], management.get_pref("client_id").unwrap());
    for key in ["unknown", "httpPort", "master_secret", "signature_key_pair"] {
        assert!(management.get_pref(key).is_err(), "{key}");
        assert!(management.get_prefs(&[key.into()]).is_err(), "{key}");
    }
    core.stop_core().await;
    assert!(management.get_pref("http_port").is_err());
    assert!(management.get_prefs(&keys).is_err());
    assert!(management.get_prefs(&[]).is_err());
}

#[tokio::test]
async fn derived_native_keys_are_readable_but_not_writable() {
    use plain_rs::prefs::pref_keys as keys;
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let management = core.native_management().unwrap();
    let before = state.prefs.entries_sorted();
    let mut events = state.events.subscribe();
    for key in [keys::UPDATE_INFO, keys::APP_UPDATE_AVAILABLE, keys::ONBOARDING_COMPLETED, keys::SIGNATURE_PUBLIC_KEY] {
        let value = management.get_pref(key).unwrap();
        assert_eq!(management.get_prefs(&[key.into()]).unwrap()[key], value);
        assert!(management.set_pref(key, value).await.is_err(), "{key}");
    }
    assert_eq!(state.prefs.entries_sorted(), before);
    assert_eq!(preference_updates(&mut events), 0);
    core.stop_core().await;
}

#[tokio::test]
async fn keyed_identity_and_permission_reads_ignore_unrelated_invalid_data() {
    use plain_rs::prefs::pref_keys as keys;
    use serde_json::json;
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let management = core.native_management().unwrap();
    let client_id = management.get_pref(keys::CLIENT_ID).unwrap();
    let url_token = management.get_pref(keys::URL_TOKEN).unwrap();
    state
        .prefs
        .set(keys::API_PERMISSIONS, vec!["CAMERA"])
        .unwrap();
    state
        .prefs
        .set(keys::EDITOR_FONT_SIZE, "invalid font size")
        .unwrap();
    state
        .prefs
        .set(keys::PASSWORD_TYPE, "invalid password type")
        .unwrap();
    state
        .prefs
        .set(keys::SIGNATURE_KEY_PAIR, "invalid signing identity")
        .unwrap();
    state
        .store
        .get()
        .unwrap()
        .set_json(keys::UPDATE_INFO, json!("invalid update info"))
        .unwrap();
    state
        .store
        .get()
        .unwrap()
        .set_json(keys::ONBOARDING_COMPLETED, json!("invalid flag"))
        .unwrap();
    assert!(management.preferences().is_err());
    assert_eq!(management.get_pref(keys::CLIENT_ID).unwrap(), client_id);
    assert_eq!(management.get_pref(keys::URL_TOKEN).unwrap(), url_token);
    assert_eq!(
        management.get_pref(keys::API_PERMISSIONS).unwrap(),
        json!(["CAMERA"])
    );
    let selected = management
        .get_prefs(&[
            keys::CLIENT_ID.into(),
            keys::URL_TOKEN.into(),
            keys::API_PERMISSIONS.into(),
        ])
        .unwrap();
    assert_eq!(selected.len(), 3);
    assert_eq!(selected[keys::CLIENT_ID], client_id);
    assert_eq!(selected[keys::URL_TOKEN], url_token);
    assert_eq!(selected[keys::API_PERMISSIONS], json!(["CAMERA"]));
    for key in [
        keys::EDITOR_FONT_SIZE,
        keys::PASSWORD_TYPE,
        keys::SIGNATURE_PUBLIC_KEY,
        keys::UPDATE_INFO,
        keys::ONBOARDING_COMPLETED,
        "unknown",
        keys::MASTER_SECRET,
        keys::SIGNATURE_KEY_PAIR,
    ] {
        assert!(management.get_pref(key).is_err(), "{key}");
        assert!(management.get_prefs(&[key.into()]).is_err(), "{key}");
    }
    core.stop_core().await;
}

#[tokio::test]
async fn derived_native_reads_use_only_their_current_dependencies() {
    use plain_rs::prefs::pref_keys as keys;
    use serde_json::json;
    let directory = tempfile::tempdir().unwrap();
    let core = ContentServer::open(directory.path()).unwrap();
    let state = core.runtime_state();
    let management = core.native_management().unwrap();
    let public_key = management.get_pref(keys::SIGNATURE_PUBLIC_KEY).unwrap();
    state
        .prefs
        .set(keys::EDITOR_FONT_SIZE, "invalid font size")
        .unwrap();
    state
        .prefs
        .set(keys::PASSWORD_TYPE, "invalid password type")
        .unwrap();
    assert_eq!(
        management.get_pref(keys::SIGNATURE_PUBLIC_KEY).unwrap(),
        public_key
    );
    assert_eq!(
        management.get_pref(keys::ONBOARDING_COMPLETED).unwrap(),
        json!(false)
    );
    state
        .store
        .get()
        .unwrap()
        .set_json(keys::ONBOARDING_COMPLETED, true)
        .unwrap();
    assert_eq!(
        management.get_pref(keys::ONBOARDING_COMPLETED).unwrap(),
        json!(true)
    );
    state
        .store
        .get()
        .unwrap()
        .set_json(keys::UPDATE_INFO, json!({"newVersion":"999.0.0"}))
        .unwrap();
    assert_eq!(
        management.get_pref(keys::UPDATE_INFO).unwrap()["newVersion"],
        "999.0.0"
    );
    assert_eq!(
        management.get_pref(keys::APP_UPDATE_AVAILABLE).unwrap(),
        json!(true)
    );
    state.prefs.set(keys::SKIP_VERSION, "999.0.0").unwrap();
    assert_eq!(
        management.get_pref(keys::APP_UPDATE_AVAILABLE).unwrap(),
        json!(false)
    );
    state.prefs.set(keys::SKIP_VERSION, 123).unwrap();
    assert!(management.get_pref(keys::APP_UPDATE_AVAILABLE).is_err());
    assert_eq!(
        management.get_pref(keys::UPDATE_INFO).unwrap()["newVersion"],
        "999.0.0"
    );
    state
        .store
        .get()
        .unwrap()
        .set_json(keys::UPDATE_INFO, json!("invalid update info"))
        .unwrap();
    assert!(management.get_pref(keys::UPDATE_INFO).is_err());
    assert_eq!(
        management.get_pref(keys::SIGNATURE_PUBLIC_KEY).unwrap(),
        public_key
    );
    assert_eq!(
        management.get_pref(keys::ONBOARDING_COMPLETED).unwrap(),
        json!(true)
    );
    core.stop_core().await;
}
