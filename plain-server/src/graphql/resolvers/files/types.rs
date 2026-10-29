use crate::graphql::types::Instant;
use crate::graphql::types::Long;
use async_graphql::Enum;
use async_graphql::ID;
use async_graphql::SimpleObject;

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, Default)]
pub enum DriveType {
    #[default]
    InternalStorage,
    Sdcard,
    UsbStorage,
    App,
}

impl DriveType {
    pub(crate) fn parse(value: &str) -> Self {
        match value {
            "SDCARD" => Self::Sdcard,
            "USB_STORAGE" => Self::UsbStorage,
            "APP" => Self::App,
            _ => Self::InternalStorage,
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct Mount {
    pub id: ID,
    pub name: String,
    pub path: String,
    #[graphql(name = "mountPoint")]
    pub mount_point: String,
    #[graphql(name = "fsType")]
    pub fs_type: String,
    #[graphql(name = "totalBytes")]
    pub total_bytes: Long,
    #[graphql(name = "usedBytes")]
    pub used_bytes: Long,
    #[graphql(name = "freeBytes")]
    pub free_bytes: Long,
    pub remote: bool,
    pub alias: String,
    #[graphql(name = "driveType")]
    pub drive_type: DriveType,
    #[graphql(name = "diskId")]
    pub disk_id: String,
}

/// One entry of a directory listing or a search page.
#[derive(SimpleObject, Clone, Debug)]
pub struct File {
    pub permission: String,
    /// Empty for non-media files — exposed as `null` rather than an empty id.
    #[graphql(name = "mediaId")]
    pub file_id: Option<ID>,
    pub name: String,
    pub path: String,
    #[graphql(name = "createdAt")]
    pub created_at: Option<Instant>,
    #[graphql(name = "updatedAt")]
    pub updated_at: Instant,
    pub size: Long,
    #[graphql(name = "isDir")]
    pub is_dir: bool,
    #[graphql(name = "childCount")]
    pub child_count: i32,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct ImageFileInfo {
    pub make: String,
    pub model: String,
    pub rotation: i32,
    pub color_space: String,
    pub aperture_value: f64,
    pub exposure_time: String,
    pub focal_length: String,
    pub iso_speed: i32,
    pub taken_at: Option<crate::graphql::types::Instant>,
    pub flash: i32,
    pub f_number: f64,
    pub exposure_program: i32,
    pub metering_mode: i32,
    pub white_balance: i32,
    pub creator: String,
    pub resolution_x: i32,
    pub resolution_y: i32,
    pub description: String,

    pub width: i32,
    pub height: i32,
    pub location: Option<Location>,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct AudioFileInfo {
    #[graphql(name = "durationMs")]
    pub duration_ms: Long,
    pub location: Option<Location>,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct VideoFileInfo {
    pub width: i32,
    pub height: i32,
    #[graphql(name = "durationMs")]
    pub duration_ms: Long,
    pub rotation: i32,
    pub display_width: i32,
    pub display_height: i32,
    pub bitrate: Long,
    pub frame_rate: f64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub taken_at: Option<Instant>,
    pub writer: String,
    pub composer: String,
    pub location: Option<Location>,
}

#[derive(async_graphql::Union, Clone, Debug)]
pub enum MediaFileInfo {
    Image(ImageFileInfo),
    Audio(AudioFileInfo),
    Video(VideoFileInfo),
}

/// Stat plus the media probe for one path. `data` is `null` when the name
/// is not a media extension, which is why it is a union and not a
/// single struct.
#[derive(SimpleObject, Clone, Debug)]
pub struct FileInfo {
    pub path: String,
    #[graphql(name = "updatedAt")]
    pub updated_at: Instant,
    pub size: Long,
    pub data: Option<MediaFileInfo>,
}

pub use plain_rs::prefs::settings::FileSortBy;
