use plain_graphql::Enum;

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum Capability {
    Adb,
    Ble,
    WifiAware,

    AppRelaunch,
    MediaTrash,
    MediaDelete,
    MediaMove,
    MediaFavorite,
    VideoRemux,
    VideoTranscode,
    AppUpdates,

    MirrorAudio,
    MediaScan,
    Telephony,
    Packages,
    ScreenMirror,
    Notifications,
    Clipboard,
    SmbServer,
    DiskManager,
}

impl Capability {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "ADB" => Self::Adb,
            "BLE" => Self::Ble,
            "WIFI_AWARE" => Self::WifiAware,

            "APP_RELAUNCH" => Self::AppRelaunch,
            "MEDIA_TRASH" => Self::MediaTrash,
            "MEDIA_DELETE" => Self::MediaDelete,
            "MEDIA_MOVE" => Self::MediaMove,
            "MEDIA_FAVORITE" => Self::MediaFavorite,
            "VIDEO_REMUX" => Self::VideoRemux,
            "VIDEO_TRANSCODE" => Self::VideoTranscode,
            "APP_UPDATES" => Self::AppUpdates,

            "MIRROR_AUDIO" => Self::MirrorAudio,
            "MEDIA_SCAN" => Self::MediaScan,
            "TELEPHONY" => Self::Telephony,
            "PACKAGES" => Self::Packages,
            "SCREEN_MIRROR" => Self::ScreenMirror,
            "NOTIFICATIONS" => Self::Notifications,
            "CLIPBOARD" => Self::Clipboard,
            "SMB_SERVER" => Self::SmbServer,
            "DISK_MANAGER" => Self::DiskManager,
            _ => return None,
        })
    }
}
