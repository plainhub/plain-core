use plain_graphql::{ID, SimpleObject};

pub use plain_rs::features::enums::MediaDataType;

pub use plain_rs::features::services::files::scanning::FileIndexScanState;

#[derive(SimpleObject, Clone, Debug, serde::Deserialize)]
pub struct FileIndexScanProgress {
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
    #[graphql(name = "topItemPaths")]
    pub top_item_paths: Vec<String>,
}
