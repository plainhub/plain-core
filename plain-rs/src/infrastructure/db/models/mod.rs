#[cfg(feature = "chat")]
mod app_file;
#[cfg(feature = "library")]
pub mod audio_queue;
#[cfg(feature = "library")]
mod audit_event;
#[cfg(feature = "chat")]
pub mod bookmark;
#[cfg(feature = "sqlite_browse")]
pub mod browse;
#[cfg(feature = "chat")]
mod channel;
#[cfg(feature = "chat")]
mod chat;
#[cfg(feature = "library")]
pub mod favorite_folder;
#[cfg(feature = "library")]
pub mod image_editor_project;
#[cfg(feature = "chat")]
mod nearby_device;
#[cfg(feature = "library")]
pub mod notes_feeds;
#[cfg(feature = "chat")]
mod peer;
#[cfg(feature = "library")]
pub mod simple;
#[cfg(feature = "library")]
pub mod tag;

#[cfg(feature = "chat")]
pub use app_file::DAppFile;
#[cfg(feature = "library")]
pub use audio_queue::{HISTORY_KEEP, Playlist, PlaylistItem, QueueItem};
#[cfg(feature = "chat")]
pub use channel::DChannel;
#[cfg(feature = "chat")]
pub use chat::DChat;
#[cfg(feature = "library")]
pub use favorite_folder::FavoriteFolderRow;
#[cfg(feature = "chat")]
pub use nearby_device::DNearbyDeviceCache;
#[cfg(feature = "chat")]
pub use peer::DPeer;
#[cfg(feature = "library")]
pub use simple::{
    ArchivedConversationRow, ClipboardRow, ImageEmbeddingRow, PomodoroItemRow, SessionRow,
    ShareRow, TrashedMessageRow,
};
#[cfg(feature = "library")]
pub use tag::{TagRelationRow, TagRow};

#[cfg(feature = "library")]
pub mod file;
#[cfg(feature = "library")]
pub mod media_file;
#[cfg(feature = "library")]
pub use file::{FileKind, FileRow, VolumeRow};
#[cfg(feature = "library")]
pub use media_file::{
    MediaFileMetadata, MediaFileMetadataAvailability, MediaFileMetadataRead, MediaFileRow,
};

#[cfg(feature = "library")]
pub use audit_event::AuditEventRow;

#[cfg(feature = "library")]
mod media_play_history;
#[cfg(feature = "library")]
pub use media_play_history::MediaPlayHistoryRow;

#[cfg(feature = "chat")]
pub(crate) fn serialize_optional_text<S: serde::Serializer>(
    value: &Option<String>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(value.as_deref().unwrap_or_default())
}

#[cfg(feature = "chat")]
pub(crate) fn deserialize_optional_text<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    <Option<String> as serde::Deserialize>::deserialize(deserializer)
        .map(|value| value.filter(|value| !value.is_empty()))
}
