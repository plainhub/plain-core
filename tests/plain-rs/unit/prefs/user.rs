use super::*;
fn fixture() -> (tempfile::TempDir, Prefs) {
    let dir = tempfile::tempdir().unwrap();
    let prefs = Prefs::load(&dir.path().join("prefs.json")).unwrap();
    (dir, prefs)
}
#[test]
fn defaults_are_complete_and_partial_patch_does_not_reset_other_settings() {
    let (_dir, prefs) = fixture();
    let settings = snapshot(&prefs).unwrap();
    assert_eq!(settings.http_port, 8080);
    assert!(settings.keep_awake);
    assert!(settings.desktop_access);
    assert_eq!(
        settings.app_home_features,
        "IMAGES|VIDEOS|AUDIO|DOCS|FILES|NOTES|FEEDS"
    );
    patch(
        &prefs,
        SettingsPatch {
            editor_font_size: Some(20),
            ..Default::default()
        },
    )
    .unwrap();
    patch(
        &prefs,
        SettingsPatch {
            keep_awake: Some(false),
            ..Default::default()
        },
    )
    .unwrap();
    let result = snapshot(&prefs).unwrap();
    assert_eq!(result.editor_font_size, 20);
    assert!(!result.keep_awake);
}

#[test]
fn invalid_patch_and_failed_write_leave_store_unchanged() {
    let (dir, prefs) = fixture();
    let before = prefs.entries();
    assert!(
        patch(
            &prefs,
            SettingsPatch {
                http_port: Some(70000),
                ..Default::default()
            }
        )
        .is_err()
    );
    assert_eq!(before, prefs.entries());
    std::fs::create_dir(dir.path().join("prefs.json.tmp")).unwrap();
    assert!(
        patch(
            &prefs,
            SettingsPatch {
                keep_awake: Some(false),
                ..Default::default()
            }
        )
        .is_err()
    );
    assert_eq!(before, prefs.entries());
}
#[test]
fn notification_actions_are_atomic_and_reject_invalid_modes() {
    let (_dir, prefs) = fixture();
    std::thread::scope(|scope| {
        for i in 0..12 {
            let prefs = &prefs;
            scope.spawn(move || {
                Action::ToggleNotificationApp(format!("app{i}"))
                    .apply(prefs)
                    .unwrap();
            });
        }
    });
    assert_eq!(
        snapshot(&prefs).unwrap().notification_filter["apps"]
            .as_array()
            .unwrap()
            .len(),
        12
    );
    Action::SetNotificationMode("allowlist".into())
        .apply(&prefs)
        .unwrap();
    Action::ClearNotificationApps.apply(&prefs).unwrap();
    assert_eq!(
        snapshot(&prefs).unwrap().notification_filter,
        serde_json::json!({"mode":"allowlist","apps":[]})
    );
    assert!(
        Action::SetNotificationMode("other".into())
            .apply(&prefs)
            .is_err()
    );
}

#[test]
fn structured_values_reject_unusable_shapes_without_writing() {
    let (_dir, prefs) = fixture();
    let before = prefs.entries();
    for input in [
        SettingsPatch {
            notification_filter: Some(serde_json::json!({"mode":"other","apps":[]})),
            ..Default::default()
        },
        SettingsPatch {
            screen_mirror_quality: Some(serde_json::json!({"mode":"HD","resolution":0})),
            ..Default::default()
        },
        SettingsPatch {
            pomodoro_settings: Some(serde_json::json!({})),
            ..Default::default()
        },
    ] {
        assert!(patch(&prefs, input).is_err());
        assert_eq!(before, prefs.entries());
    }
}

#[test]
fn listener_ports_use_the_single_preferences_store() {
    let (_directory, prefs) = fixture();
    prefs.set("http_port", 9090).unwrap();
    prefs.set("https_port", 9443).unwrap();
    let initial = snapshot(&prefs).unwrap();
    assert_eq!((initial.http_port, initial.https_port), (9090, 9443));
    patch(
        &prefs,
        SettingsPatch {
            http_port: Some(8180),
            https_port: Some(8543),
            ..Default::default()
        },
    )
    .unwrap();
    let configured = snapshot(&prefs).unwrap();
    assert_eq!((configured.http_port, configured.https_port), (8180, 8543));
    assert_eq!(prefs.get::<i32>("http_port").unwrap(), Some(8180));
    assert_eq!(prefs.get::<i32>("https_port").unwrap(), Some(8543));
}

#[test]
fn keyed_values_include_defaults_without_writing_and_round_trip_through_validated_entries() {
    let (_dir, prefs) = fixture();
    let stored = prefs.entries();
    let initial = values(&prefs).unwrap();
    assert_eq!(initial[keys::HTTP_PORT], 8080);
    assert_eq!(initial[keys::FEED_AUTO_REFRESH_INTERVAL], 7200);
    assert_eq!(initial[keys::PACKAGE_SORT_BY], "NAME_ASC");
    assert_eq!(prefs.entries(), stored);
    for key in initial.keys() {
        assert!(keys::access(key).is_some(), "{key}");
    }
    for (key, value) in [
        (keys::HTTP_PORT, serde_json::json!(8181)),
        (keys::FEED_AUTO_REFRESH_INTERVAL, serde_json::json!(900)),
        (keys::PACKAGE_SORT_BY, serde_json::json!("NAME_DESC")),
    ] {
        assert_eq!(set_entry(&prefs, key, value.clone()).unwrap(), Some(true));
        assert_eq!(values(&prefs).unwrap()[key], value);
        assert_eq!(prefs.get::<Value>(key).unwrap(), Some(value.clone()));
        assert_eq!(set_entry(&prefs, key, value).unwrap(), Some(false));
    }
    let before_invalid = prefs.entries();
    assert!(set_entry(&prefs, keys::HTTP_PORT, serde_json::json!(70000)).is_err());
    assert!(set_entry(&prefs, keys::HTTP_PORT, serde_json::json!("invalid")).is_err());
    assert_eq!(
        set_entry(&prefs, keys::HTTP_PORT, Value::Null).unwrap(),
        Some(false)
    );
    assert_eq!(prefs.entries(), before_invalid);
    assert_eq!(
        set_entry(&prefs, "httpPort", serde_json::json!(8181)).unwrap(),
        None
    );
}

#[test]
fn keyed_reads_use_core_defaults_and_do_not_decode_unrelated_preferences() {
    let (_dir, prefs) = fixture();
    let before = prefs.entries();
    for (key, value) in values(&prefs).unwrap() {
        assert_eq!(get_entry(&prefs, &key).unwrap(), Some(value));
    }
    assert_eq!(prefs.entries(), before);
    prefs.set(keys::SHOW_HIDDEN_FILES, "invalid").unwrap();
    assert_eq!(
        get_entry(&prefs, keys::HTTP_PORT).unwrap(),
        Some(serde_json::json!(8080))
    );
    assert!(get_entry(&prefs, keys::SHOW_HIDDEN_FILES).is_err());
    assert_eq!(get_entry(&prefs, "unknown").unwrap(), None);
}
