use plain_graphql::Enum;

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum Capability {
    Adb,
    DirectoryAccess,
    Ble,
    WifiAware,
    BackgroundService,
    SoundMeter,

    FileExport,
    FileReveal,
    FileClipboard,
    AppRelaunch,
    MediaTrash,
    MediaDelete,
    MediaMove,
    MediaFavorite,
    VideoRemux,
    VideoTranscode,
    AppUpdates,

    MirrorAudio,
    DocPreview,
    ImageSearch,
    MediaScan,
    Telephony,
    Packages,
    ScreenMirror,
    ImageEditor,
    Notifications,
    Clipboard,
    LanShare,
    DiskManager,
}

impl Capability {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "ADB" => Self::Adb,
            "DIRECTORY_ACCESS" => Self::DirectoryAccess,
            "BLE" => Self::Ble,
            "WIFI_AWARE" => Self::WifiAware,
            "BACKGROUND_SERVICE" => Self::BackgroundService,
            "SOUND_METER" => Self::SoundMeter,

            "FILE_EXPORT" => Self::FileExport,
            "FILE_REVEAL" => Self::FileReveal,
            "FILE_CLIPBOARD" => Self::FileClipboard,
            "APP_RELAUNCH" => Self::AppRelaunch,
            "MEDIA_TRASH" => Self::MediaTrash,
            "MEDIA_DELETE" => Self::MediaDelete,
            "MEDIA_MOVE" => Self::MediaMove,
            "MEDIA_FAVORITE" => Self::MediaFavorite,
            "VIDEO_REMUX" => Self::VideoRemux,
            "VIDEO_TRANSCODE" => Self::VideoTranscode,
            "APP_UPDATES" => Self::AppUpdates,

            "MIRROR_AUDIO" => Self::MirrorAudio,
            "DOC_PREVIEW" => Self::DocPreview,
            "IMAGE_SEARCH" => Self::ImageSearch,
            "MEDIA_SCAN" => Self::MediaScan,
            "TELEPHONY" => Self::Telephony,
            "PACKAGES" => Self::Packages,
            "SCREEN_MIRROR" => Self::ScreenMirror,
            "IMAGE_EDITOR" => Self::ImageEditor,
            "NOTIFICATIONS" => Self::Notifications,
            "CLIPBOARD" => Self::Clipboard,
            "LAN_SHARE" => Self::LanShare,
            "DISK_MANAGER" => Self::DiskManager,
            _ => return None,
        })
    }
}
