#[derive(serde::Serialize, serde::Deserialize, Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(feature = "graphql", derive(plain_graphql::Enum))]
pub enum Capability {
    Adb,
    Ble,
    WifiAware,
    AppRelaunch,
    MediaMove,
    MediaFavorite,
    AppUpdates,
    MirrorAudio,
    MediaScan,
    Telephony,
    Packages,
    ScreenMirror,
    Notifications,
    Clipboard,
    DiskManager,
    HttpServer,
}

impl Capability {
    pub const ALL: &[Self] = &[
        Self::Adb,
        Self::Ble,
        Self::WifiAware,
        Self::AppRelaunch,
        Self::MediaMove,
        Self::MediaFavorite,
        Self::AppUpdates,
        Self::MirrorAudio,
        Self::MediaScan,
        Self::Telephony,
        Self::Packages,
        Self::ScreenMirror,
        Self::Notifications,
        Self::Clipboard,
        Self::DiskManager,
        Self::HttpServer,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Adb => "ADB",
            Self::Ble => "BLE",
            Self::WifiAware => "WIFI_AWARE",
            Self::AppRelaunch => "APP_RELAUNCH",
            Self::MediaMove => "MEDIA_MOVE",
            Self::MediaFavorite => "MEDIA_FAVORITE",
            Self::AppUpdates => "APP_UPDATES",
            Self::MirrorAudio => "MIRROR_AUDIO",
            Self::MediaScan => "MEDIA_SCAN",
            Self::Telephony => "TELEPHONY",
            Self::Packages => "PACKAGES",
            Self::ScreenMirror => "SCREEN_MIRROR",
            Self::Notifications => "NOTIFICATIONS",
            Self::Clipboard => "CLIPBOARD",
            Self::DiskManager => "DISK_MANAGER",
            Self::HttpServer => "HTTP_SERVER",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|capability| capability.as_str() == value)
    }
}

impl std::fmt::Display for Capability {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

pub fn deserialize_list<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<Capability>, D::Error> {
    use serde::Deserialize;
    Ok(Vec::<String>::deserialize(deserializer)?
        .into_iter()
        .filter_map(|name| Capability::parse(&name))
        .collect())
}

#[cfg(test)]
#[path = "../../../tests/plain-rs/unit/features/capability.rs"]
mod tests;
