mod enum_value;
mod sort;
pub use sort::{AudioSortBy, DocSortBy, FileSortBy, ImageSortBy, MediaSortBy, VideoSortBy};
mod play_mode;
pub use play_mode::MediaPlayMode;
mod theme;
pub use theme::DarkTheme;
mod entries;
pub use entries::{get_entry, set_entry, values};
mod actions;
use super::pref_keys as keys;
mod patch;
mod state;
mod validation;
use super::{Prefs, PrefsError, Result};
pub use actions::Action;
pub use patch::SettingsPatch;
use plain_libs::serde_json::{Map, Value};
pub use state::Settings;
use validation::validate;

pub fn snapshot(prefs: &Prefs) -> Result<Settings> {
    let entries: Map<String, Value> = prefs.entries().into_iter().collect();
    let mut settings = Settings::default();
    if let Some(value) = entries.get(keys::HTTP_PORT) {
        settings.http_port = plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::HTTPS_PORT) {
        settings.https_port = plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::DARK_THEME) {
        settings.dark_theme = plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::AMOLED_DARK_THEME) {
        settings.amoled_dark_theme =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::PDF_FOLLOW_DARK_THEME) {
        settings.pdf_follow_dark_theme =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::LOCALE) {
        settings.locale = plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::BACKGROUND_SERVICE_ENABLED) {
        settings.background_service_enabled =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::REMOTE_ACCESS) {
        settings.remote_access =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::DLNA) {
        settings.dlna = plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::DEVELOPER_MODE) {
        settings.developer_mode =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::ALLOW_ANY_HOST) {
        settings.allow_any_host =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::DEVICE_NAME) {
        settings.device_name = plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::HTTPS) {
        settings.https = plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::WEB_ADDRESS_BAR_EXPANDED) {
        settings.web_address_bar_expanded =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::SCREEN_MIRROR_QUALITY) {
        settings.screen_mirror_quality =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::AUDIO_PLAY_MODE) {
        settings.audio_play_mode =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::AUDIO_PLAYBACK_SPEED) {
        settings.audio_playback_speed =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::IMAGE_GRID_CELLS_PER_ROW) {
        settings.image_grid_cells_per_row =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::VIDEO_GRID_CELLS_PER_ROW) {
        settings.video_grid_cells_per_row =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::SHOW_HIDDEN_FILES) {
        settings.show_hidden_files =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::FEED_AUTO_REFRESH) {
        settings.feed_auto_refresh =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::FEED_AUTO_REFRESH_INTERVAL) {
        settings.feed_auto_refresh_interval =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::FEED_AUTO_REFRESH_ONLY_WIFI) {
        settings.feed_auto_refresh_only_wifi =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::FEED_FONT_SCALE) {
        settings.feed_font_scale =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::EDITOR_WRAP_CONTENT) {
        settings.editor_wrap_content =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::EDITOR_FONT_SIZE) {
        settings.editor_font_size =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::EDITOR_STATUS_BAR) {
        settings.editor_status_bar =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }

    if let Some(value) = entries.get(keys::NEARBY_DISCOVERABLE) {
        settings.nearby_discoverable =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::AI_IMAGE_SEARCH_ENABLED) {
        settings.ai_image_search_enabled =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::NOTIFICATION_FILTER) {
        settings.notification_filter =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::POMODORO_SETTINGS) {
        settings.pomodoro_settings =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }

    if let Some(value) = entries.get(keys::APP_HOME_FEATURES) {
        settings.app_home_features =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }

    if let Some(value) = entries.get(keys::LAUNCHER_SHORTCUTS) {
        settings.launcher_shortcuts =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }

    if let Some(value) = entries.get(keys::AUDIO_SORT_BY) {
        settings.audio_sort_by =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::VIDEO_SORT_BY) {
        settings.video_sort_by =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::IMAGE_SORT_BY) {
        settings.image_sort_by =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::DOC_SORT_BY) {
        settings.doc_sort_by = plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::FILE_SORT_BY) {
        settings.file_sort_by =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    if let Some(value) = entries.get(keys::PACKAGE_SORT_BY) {
        settings.package_sort_by =
            plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
    }
    Ok(settings)
}

pub fn patch(prefs: &Prefs, patch: SettingsPatch) -> plain_libs::anyhow::Result<bool> {
    validate(&patch)?;
    let mut changed = false;
    prefs.update(|entries| {
        if let Some(value) = patch.http_port {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::HTTP_PORT) != Some(&value) {
                entries.insert(keys::HTTP_PORT.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.https_port {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::HTTPS_PORT) != Some(&value) {
                entries.insert(keys::HTTPS_PORT.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.dark_theme {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::DARK_THEME) != Some(&value) {
                entries.insert(keys::DARK_THEME.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.amoled_dark_theme {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::AMOLED_DARK_THEME) != Some(&value) {
                entries.insert(keys::AMOLED_DARK_THEME.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.pdf_follow_dark_theme {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::PDF_FOLLOW_DARK_THEME) != Some(&value) {
                entries.insert(keys::PDF_FOLLOW_DARK_THEME.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.locale {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::LOCALE) != Some(&value) {
                entries.insert(keys::LOCALE.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.background_service_enabled {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::BACKGROUND_SERVICE_ENABLED) != Some(&value) {
                entries.insert(keys::BACKGROUND_SERVICE_ENABLED.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.remote_access {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::REMOTE_ACCESS) != Some(&value) {
                entries.insert(keys::REMOTE_ACCESS.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.dlna {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::DLNA) != Some(&value) {
                entries.insert(keys::DLNA.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.developer_mode {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::DEVELOPER_MODE) != Some(&value) {
                entries.insert(keys::DEVELOPER_MODE.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.allow_any_host {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::ALLOW_ANY_HOST) != Some(&value) {
                entries.insert(keys::ALLOW_ANY_HOST.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.device_name {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::DEVICE_NAME) != Some(&value) {
                entries.insert(keys::DEVICE_NAME.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.https {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::HTTPS) != Some(&value) {
                entries.insert(keys::HTTPS.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.web_address_bar_expanded {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::WEB_ADDRESS_BAR_EXPANDED) != Some(&value) {
                entries.insert(keys::WEB_ADDRESS_BAR_EXPANDED.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.screen_mirror_quality {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::SCREEN_MIRROR_QUALITY) != Some(&value) {
                entries.insert(keys::SCREEN_MIRROR_QUALITY.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.audio_play_mode {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::AUDIO_PLAY_MODE) != Some(&value) {
                entries.insert(keys::AUDIO_PLAY_MODE.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.audio_playback_speed {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::AUDIO_PLAYBACK_SPEED) != Some(&value) {
                entries.insert(keys::AUDIO_PLAYBACK_SPEED.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.image_grid_cells_per_row {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::IMAGE_GRID_CELLS_PER_ROW) != Some(&value) {
                entries.insert(keys::IMAGE_GRID_CELLS_PER_ROW.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.video_grid_cells_per_row {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::VIDEO_GRID_CELLS_PER_ROW) != Some(&value) {
                entries.insert(keys::VIDEO_GRID_CELLS_PER_ROW.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.show_hidden_files {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::SHOW_HIDDEN_FILES) != Some(&value) {
                entries.insert(keys::SHOW_HIDDEN_FILES.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.feed_auto_refresh {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::FEED_AUTO_REFRESH) != Some(&value) {
                entries.insert(keys::FEED_AUTO_REFRESH.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.feed_auto_refresh_interval {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::FEED_AUTO_REFRESH_INTERVAL) != Some(&value) {
                entries.insert(keys::FEED_AUTO_REFRESH_INTERVAL.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.feed_auto_refresh_only_wifi {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::FEED_AUTO_REFRESH_ONLY_WIFI) != Some(&value) {
                entries.insert(keys::FEED_AUTO_REFRESH_ONLY_WIFI.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.feed_font_scale {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::FEED_FONT_SCALE) != Some(&value) {
                entries.insert(keys::FEED_FONT_SCALE.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.editor_wrap_content {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::EDITOR_WRAP_CONTENT) != Some(&value) {
                entries.insert(keys::EDITOR_WRAP_CONTENT.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.editor_font_size {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::EDITOR_FONT_SIZE) != Some(&value) {
                entries.insert(keys::EDITOR_FONT_SIZE.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.editor_status_bar {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::EDITOR_STATUS_BAR) != Some(&value) {
                entries.insert(keys::EDITOR_STATUS_BAR.into(), value);
                changed = true;
            }
        }

        if let Some(value) = patch.nearby_discoverable {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::NEARBY_DISCOVERABLE) != Some(&value) {
                entries.insert(keys::NEARBY_DISCOVERABLE.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.ai_image_search_enabled {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::AI_IMAGE_SEARCH_ENABLED) != Some(&value) {
                entries.insert(keys::AI_IMAGE_SEARCH_ENABLED.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.notification_filter {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::NOTIFICATION_FILTER) != Some(&value) {
                entries.insert(keys::NOTIFICATION_FILTER.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.pomodoro_settings {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::POMODORO_SETTINGS) != Some(&value) {
                entries.insert(keys::POMODORO_SETTINGS.into(), value);
                changed = true;
            }
        }

        if let Some(value) = patch.app_home_features {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::APP_HOME_FEATURES) != Some(&value) {
                entries.insert(keys::APP_HOME_FEATURES.into(), value);
                changed = true;
            }
        }

        if let Some(value) = patch.launcher_shortcuts {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::LAUNCHER_SHORTCUTS) != Some(&value) {
                entries.insert(keys::LAUNCHER_SHORTCUTS.into(), value);
                changed = true;
            }
        }

        if let Some(value) = patch.audio_sort_by {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::AUDIO_SORT_BY) != Some(&value) {
                entries.insert(keys::AUDIO_SORT_BY.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.video_sort_by {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::VIDEO_SORT_BY) != Some(&value) {
                entries.insert(keys::VIDEO_SORT_BY.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.image_sort_by {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::IMAGE_SORT_BY) != Some(&value) {
                entries.insert(keys::IMAGE_SORT_BY.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.doc_sort_by {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::DOC_SORT_BY) != Some(&value) {
                entries.insert(keys::DOC_SORT_BY.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.file_sort_by {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::FILE_SORT_BY) != Some(&value) {
                entries.insert(keys::FILE_SORT_BY.into(), value);
                changed = true;
            }
        }
        if let Some(value) = patch.package_sort_by {
            let value = plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?;
            if entries.get(keys::PACKAGE_SORT_BY) != Some(&value) {
                entries.insert(keys::PACKAGE_SORT_BY.into(), value);
                changed = true;
            }
        }
        Ok(())
    })?;
    Ok(changed)
}
#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/prefs/user.rs"]
mod tests;
