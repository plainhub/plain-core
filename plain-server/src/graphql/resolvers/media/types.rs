use plain_graphql::{ID, SimpleObject};

pub use plain_rs::features::enums::MediaDataType;

pub use plain_rs::features::services::files::scanning::FileIndexScanState;

#[derive(SimpleObject, Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileIndexScanProgress {
    pub volume_id: Option<ID>,
    pub indexed: i32,
    pub pending: Option<i32>,
    pub total: Option<i32>,
    pub state: FileIndexScanState,
    pub error: Option<String>,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct MediaBucket {
    pub size: plain_rs::infrastructure::types::Long,
    pub id: ID,
    pub name: String,
    #[graphql(name = "itemCount")]
    pub item_count: i32,
    pub cover_items: Vec<MediaBucketCover>,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct MediaBucketCover {
    pub path: String,
    pub updated_at: plain_rs::infrastructure::types::Instant,
    pub thumb_hash: Option<String>,
}
