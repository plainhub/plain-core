pub mod mutation;
pub mod query;

use super::item::MediaItem;
use plain_graphql::{Enum, ID, SimpleObject};
use plain_rs::features::enums::DataType;
use plain_rs::infrastructure::types::{Instant, Long};

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum PlaybackType {
    Audio,
    Video,
}

impl PlaybackType {
    fn data_type(self) -> DataType {
        match self {
            Self::Audio => DataType::Audio,
            Self::Video => DataType::Video,
        }
    }
}

#[derive(SimpleObject)]
pub struct MediaPlayHistory {
    #[graphql(name = "mediaId")]
    file_id: ID,
    media_type: PlaybackType,
    position_ms: Long,
    play_count: Long,
    played_at: Instant,
    media: MediaItem,
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-server/unit/graphql/resolvers/media/play_history/mod.rs"]
mod tests;
