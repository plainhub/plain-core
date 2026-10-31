use super::*;

#[test]
fn every_registered_key_has_the_expected_read_and_write_policy() {
    let groups = [
        (PrefAccess::Default, "dlna device_name feed_auto_refresh feed_auto_refresh_interval feed_auto_refresh_only_wifi
            ai_image_search_enabled ai_image_search_model dlna_allowed_senders dlna_denied_senders
            clipboard_directions media_source_dirs samba_settings dark_theme amoled_dark_theme
            pdf_follow_dark_theme locale show_hidden_files feed_font_scale editor_wrap_content
            editor_font_size editor_status_bar desktop_home_features audio_sort_by video_sort_by
            image_sort_by doc_sort_by file_sort_by pkg_sort_by audio_play_mode audio_playback_speed
            pomodoro_settings screen_mirror_quality volume_alias"),
        (PrefAccess::LocalOnly, "http_port https_port keep_awake background_service_enabled desktop_access developer_mode
            allow_any_host nearby_discoverable notification_filter launcher_shortcuts
            image_grid_cells_per_row video_grid_cells_per_row https web_address_bar_expanded
            password password_type login_require_approval rotate_url_token_on_restart
            api_permissions mdns_hostname dev_token log_level auto_check_update skip_version"),
        (PrefAccess::RemoteReadOnly, "app_home_features"),
        (PrefAccess::RustOnly, "client_id url_token adb_token master_secret signature_key_pair"),
    ];
    let expected: Vec<_> = groups
        .iter()
        .flat_map(|(policy, keys)| keys.split_ascii_whitespace().map(|key| (key, *policy)))
        .collect();
    assert_eq!(PREF_KEYS, expected);
    for (key, policy) in expected {
        assert_eq!(access(key), Some(policy), "{key}");
        assert_eq!(readable(key, false), matches!(policy, PrefAccess::Default | PrefAccess::RemoteReadOnly), "{key}");
        assert_eq!(readable(key, true), policy != PrefAccess::RustOnly, "{key}");
        assert_eq!(
            writable(key, false),
            policy == PrefAccess::Default,
            "{key}"
        );
        assert_eq!(
            writable(key, true),
            policy != PrefAccess::RustOnly,
            "{key}"
        );
    }
}

#[test]
fn exported_constants_match_the_registered_keys() {
    let constants = [
        DLNA,
        DEVICE_NAME,
        FEED_AUTO_REFRESH,
        FEED_AUTO_REFRESH_INTERVAL,
        FEED_AUTO_REFRESH_ONLY_WIFI,
        AI_IMAGE_SEARCH_ENABLED,
        AI_IMAGE_SEARCH_MODEL,
        DLNA_ALLOWED_SENDERS,
        DLNA_DENIED_SENDERS,
        CLIPBOARD_DIRECTIONS,
        MEDIA_SOURCE_DIRS,
        SAMBA_SETTINGS,
        DARK_THEME,
        AMOLED_DARK_THEME,
        PDF_FOLLOW_DARK_THEME,
        LOCALE,
        SHOW_HIDDEN_FILES,
        FEED_FONT_SCALE,
        EDITOR_WRAP_CONTENT,
        EDITOR_FONT_SIZE,
        EDITOR_STATUS_BAR,
        DESKTOP_HOME_FEATURES,
        AUDIO_SORT_BY,
        VIDEO_SORT_BY,
        IMAGE_SORT_BY,
        DOC_SORT_BY,
        FILE_SORT_BY,
        PACKAGE_SORT_BY,
        AUDIO_PLAY_MODE,
        AUDIO_PLAYBACK_SPEED,
        POMODORO_SETTINGS,
        SCREEN_MIRROR_QUALITY,
        VOLUME_ALIAS,
        HTTP_PORT,
        HTTPS_PORT,
        KEEP_AWAKE,
        BACKGROUND_SERVICE_ENABLED,
        DESKTOP_ACCESS,
        DEVELOPER_MODE,
        ALLOW_ANY_HOST,
        NEARBY_DISCOVERABLE,
        NOTIFICATION_FILTER,
        LAUNCHER_SHORTCUTS,
        IMAGE_GRID_CELLS_PER_ROW,
        VIDEO_GRID_CELLS_PER_ROW,
        HTTPS,
        WEB_ADDRESS_BAR_EXPANDED,
        PASSWORD,
        PASSWORD_TYPE,
        LOGIN_REQUIRE_APPROVAL,
        ROTATE_URL_TOKEN_ON_RESTART,
        API_PERMISSIONS,
        MDNS_HOSTNAME,
        DEV_TOKEN,
        LOG_LEVEL,
        AUTO_CHECK_UPDATE,
        SKIP_VERSION,
        APP_HOME_FEATURES,
        CLIENT_ID,        URL_TOKEN,
        ADB_TOKEN,
        MASTER_SECRET,
        SIGNATURE_KEY_PAIR,
    ];
    assert_eq!(constants, PREF_KEYS.iter().map(|(key, _)| *key).collect::<Vec<_>>().as_slice());
}

#[test]
fn unknown_or_modified_keys_never_gain_access() {
    for key in [
        "",
        "unknown",
        "LOCALE",
        " locale",
        "locale ",
        "admin:password",
    ] {
        assert_eq!(access(key), None, "{key}");
        assert!(!readable(key, false), "{key}");
        assert!(!readable(key, true), "{key}");
        assert!(!writable(key, false), "{key}");
        assert!(!writable(key, true), "{key}");
    }
}

#[test]
fn native_read_keys_do_not_allow_generic_preference_writes() {
    for key in [UPDATE_INFO, APP_UPDATE_AVAILABLE, ONBOARDING_COMPLETED, SIGNATURE_PUBLIC_KEY] {
        assert_eq!(access(key), None, "{key}");
        assert!(!readable(key, false), "{key}");
        assert!(!readable(key, true), "{key}");
        assert!(!writable(key, false), "{key}");
        assert!(!writable(key, true), "{key}");
    }
}
