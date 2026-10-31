use super::{Prefs, PrefsError, Result, SettingsPatch, keys};
use serde_json::{Map, Value};

macro_rules! settings_keys {
    ($($key:ident => $field:ident),* $(,)?) => {
        pub fn values(prefs: &Prefs) -> Result<Map<String, Value>> {
            let super::Settings { $($field,)* } = super::snapshot(prefs)?;
            Ok(Map::from_iter([$(
                (keys::$key.to_owned(), serde_json::to_value($field).map_err(PrefsError::Encode)?),
            )*]))
        }

        pub fn get_entry(prefs: &Prefs, key: &str) -> Result<Option<Value>> {
            match key {
                $(keys::$key => {
                    let value = prefs.get(key)?.unwrap_or_else(|| super::Settings::default().$field);
                    Ok(Some(serde_json::to_value(value).map_err(PrefsError::Encode)?))
                },)*
                _ => Ok(None),
            }
        }

        pub fn set_entry(prefs: &Prefs, key: &str, value: Value) -> anyhow::Result<Option<bool>> {
            let patch = match key {
                $(keys::$key => SettingsPatch {
                    $field: serde_json::from_value(value)?,
                    ..Default::default()
                },)*
                _ => return Ok(None),
            };
            Ok(Some(super::patch(prefs, patch)?))
        }
    };
}

settings_keys! {
    HTTP_PORT => http_port,
    HTTPS_PORT => https_port,
    DARK_THEME => dark_theme,
    AMOLED_DARK_THEME => amoled_dark_theme,
    PDF_FOLLOW_DARK_THEME => pdf_follow_dark_theme,
    KEEP_AWAKE => keep_awake,
    LOCALE => locale,
    BACKGROUND_SERVICE_ENABLED => background_service_enabled,
    DESKTOP_ACCESS => desktop_access,
    DLNA => dlna,
    DEVELOPER_MODE => developer_mode,
    ALLOW_ANY_HOST => allow_any_host,
    DEVICE_NAME => device_name,
    HTTPS => https,
    WEB_ADDRESS_BAR_EXPANDED => web_address_bar_expanded,
    SCREEN_MIRROR_QUALITY => screen_mirror_quality,
    AUDIO_PLAY_MODE => audio_play_mode,
    AUDIO_PLAYBACK_SPEED => audio_playback_speed,
    IMAGE_GRID_CELLS_PER_ROW => image_grid_cells_per_row,
    VIDEO_GRID_CELLS_PER_ROW => video_grid_cells_per_row,
    SHOW_HIDDEN_FILES => show_hidden_files,
    FEED_AUTO_REFRESH => feed_auto_refresh,
    FEED_AUTO_REFRESH_INTERVAL => feed_auto_refresh_interval,
    FEED_AUTO_REFRESH_ONLY_WIFI => feed_auto_refresh_only_wifi,
    FEED_FONT_SCALE => feed_font_scale,
    EDITOR_WRAP_CONTENT => editor_wrap_content,
    EDITOR_FONT_SIZE => editor_font_size,
    EDITOR_STATUS_BAR => editor_status_bar,
    NEARBY_DISCOVERABLE => nearby_discoverable,
    AI_IMAGE_SEARCH_ENABLED => ai_image_search_enabled,
    NOTIFICATION_FILTER => notification_filter,
    POMODORO_SETTINGS => pomodoro_settings,
    APP_HOME_FEATURES => app_home_features,
    LAUNCHER_SHORTCUTS => launcher_shortcuts,
    AUDIO_SORT_BY => audio_sort_by,
    VIDEO_SORT_BY => video_sort_by,
    IMAGE_SORT_BY => image_sort_by,
    DOC_SORT_BY => doc_sort_by,
    FILE_SORT_BY => file_sort_by,
    PACKAGE_SORT_BY => package_sort_by,
}
