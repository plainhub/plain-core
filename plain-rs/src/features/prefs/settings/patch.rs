use super::{DarkTheme, FileSortBy, MediaPlayMode};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsPatch {
    pub http_port: Option<i32>,
    pub https_port: Option<i32>,
    pub dark_theme: Option<DarkTheme>,
    pub amoled_dark_theme: Option<bool>,
    pub pdf_follow_dark_theme: Option<bool>,
    pub keep_awake: Option<bool>,
    pub locale: Option<String>,
    pub background_service_enabled: Option<bool>,
    pub desktop_access: Option<bool>,
    pub dlna: Option<bool>,
    pub developer_mode: Option<bool>,
    pub allow_any_host: Option<bool>,
    pub device_name: Option<String>,
    pub https: Option<bool>,
    pub web_address_bar_expanded: Option<bool>,
    pub screen_mirror_quality: Option<Value>,
    pub audio_play_mode: Option<MediaPlayMode>,
    pub audio_playback_speed: Option<f32>,
    pub image_grid_cells_per_row: Option<i32>,
    pub video_grid_cells_per_row: Option<i32>,
    pub show_hidden_files: Option<bool>,
    pub feed_auto_refresh: Option<bool>,
    #[serde(rename = "feedAutoRefreshIntervalSec")]
    pub feed_auto_refresh_interval: Option<i32>,
    pub feed_auto_refresh_only_wifi: Option<bool>,
    pub feed_font_scale: Option<i32>,
    pub editor_wrap_content: Option<bool>,
    pub editor_font_size: Option<i32>,
    pub editor_status_bar: Option<bool>,
    pub nearby_discoverable: Option<bool>,
    pub ai_image_search_enabled: Option<bool>,
    pub notification_filter: Option<Value>,
    pub pomodoro_settings: Option<Value>,
    pub app_home_features: Option<String>,
    pub launcher_shortcuts: Option<String>,
    pub audio_sort_by: Option<FileSortBy>,
    pub video_sort_by: Option<FileSortBy>,
    pub image_sort_by: Option<FileSortBy>,
    pub doc_sort_by: Option<FileSortBy>,
    pub file_sort_by: Option<FileSortBy>,
    pub package_sort_by: Option<FileSortBy>,
}
