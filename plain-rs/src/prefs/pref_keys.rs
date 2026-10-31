#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefAccess {
    Default,
    LocalOnly,
    RemoteReadOnly,
    RustOnly,
}

macro_rules! pref_keys {
    ($($access:ident { $($name:ident = $key:literal),* $(,)? })*) => {
        $($(pub const $name: &str = $key;)*)*
        pub const PREF_KEYS: &[(&str, PrefAccess)] = &[$($(($key, PrefAccess::$access),)*)*];
    };
}

pref_keys! {
    Default {
        DLNA = "dlna",
        DEVICE_NAME = "device_name",
        FEED_AUTO_REFRESH = "feed_auto_refresh",
        FEED_AUTO_REFRESH_INTERVAL = "feed_auto_refresh_interval",
        FEED_AUTO_REFRESH_ONLY_WIFI = "feed_auto_refresh_only_wifi",
        AI_IMAGE_SEARCH_ENABLED = "ai_image_search_enabled",
        AI_IMAGE_SEARCH_MODEL = "ai_image_search_model",
        DLNA_ALLOWED_SENDERS = "dlna_allowed_senders",
        DLNA_DENIED_SENDERS = "dlna_denied_senders",
        CLIPBOARD_DIRECTIONS = "clipboard_directions",
        MEDIA_SOURCE_DIRS = "media_source_dirs",
        SAMBA_SETTINGS = "samba_settings",
        DARK_THEME = "dark_theme",
        AMOLED_DARK_THEME = "amoled_dark_theme",
        PDF_FOLLOW_DARK_THEME = "pdf_follow_dark_theme",
        LOCALE = "locale",
        SHOW_HIDDEN_FILES = "show_hidden_files",
        FEED_FONT_SCALE = "feed_font_scale",
        EDITOR_WRAP_CONTENT = "editor_wrap_content",
        EDITOR_FONT_SIZE = "editor_font_size",
        EDITOR_STATUS_BAR = "editor_status_bar",
        DESKTOP_HOME_FEATURES = "desktop_home_features",
        AUDIO_SORT_BY = "audio_sort_by",
        VIDEO_SORT_BY = "video_sort_by",
        IMAGE_SORT_BY = "image_sort_by",
        DOC_SORT_BY = "doc_sort_by",
        FILE_SORT_BY = "file_sort_by",
        PACKAGE_SORT_BY = "pkg_sort_by",
        AUDIO_PLAY_MODE = "audio_play_mode",
        AUDIO_PLAYBACK_SPEED = "audio_playback_speed",
        POMODORO_SETTINGS = "pomodoro_settings",
        SCREEN_MIRROR_QUALITY = "screen_mirror_quality",
        VOLUME_ALIAS = "volume_alias",
    }
    LocalOnly {
        HTTP_PORT = "http_port",
        HTTPS_PORT = "https_port",
        KEEP_AWAKE = "keep_awake",
        BACKGROUND_SERVICE_ENABLED = "background_service_enabled",
        DESKTOP_ACCESS = "desktop_access",
        DEVELOPER_MODE = "developer_mode",
        ALLOW_ANY_HOST = "allow_any_host",
        NEARBY_DISCOVERABLE = "nearby_discoverable",
        NOTIFICATION_FILTER = "notification_filter",
        LAUNCHER_SHORTCUTS = "launcher_shortcuts",
        IMAGE_GRID_CELLS_PER_ROW = "image_grid_cells_per_row",
        VIDEO_GRID_CELLS_PER_ROW = "video_grid_cells_per_row",
        HTTPS = "https",
        WEB_ADDRESS_BAR_EXPANDED = "web_address_bar_expanded",
        PASSWORD = "password",
        PASSWORD_TYPE = "password_type",
        LOGIN_REQUIRE_APPROVAL = "login_require_approval",
        ROTATE_URL_TOKEN_ON_RESTART = "rotate_url_token_on_restart",
        API_PERMISSIONS = "api_permissions",
        MDNS_HOSTNAME = "mdns_hostname",
        DEV_TOKEN = "dev_token",
        LOG_LEVEL = "log_level",
        AUTO_CHECK_UPDATE = "auto_check_update",
        SKIP_VERSION = "skip_version",
    }
    RemoteReadOnly {
        APP_HOME_FEATURES = "app_home_features",
    }
    RustOnly {
        CLIENT_ID = "client_id",
        URL_TOKEN = "url_token",
        ADB_TOKEN = "adb_token",
        MASTER_SECRET = "master_secret",
        SIGNATURE_KEY_PAIR = "signature_key_pair",
    }
}

pub const UPDATE_INFO: &str = "update_info";
pub const APP_UPDATE_AVAILABLE: &str = "app_update_available";
pub const ONBOARDING_COMPLETED: &str = "onboarding_completed";
pub const SIGNATURE_PUBLIC_KEY: &str = "signature_public_key";

pub fn access(key: &str) -> Option<PrefAccess> {
    PREF_KEYS
        .iter()
        .find_map(|(name, access)| (*name == key).then_some(*access))
}
pub fn readable(key: &str, local: bool) -> bool {
    match access(key) {
        Some(PrefAccess::Default | PrefAccess::RemoteReadOnly) => true,
        Some(PrefAccess::LocalOnly) => local,
        _ => false,
    }
}
pub fn writable(key: &str, local: bool) -> bool {
    match access(key) {
        Some(PrefAccess::Default) => true,
        Some(PrefAccess::LocalOnly | PrefAccess::RemoteReadOnly) => local,
        _ => false,
    }
}

#[cfg(test)]
#[path = "../../tests/unit/prefs/pref_keys.rs"]
mod tests;
