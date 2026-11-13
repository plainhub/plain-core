pub mod queue;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum Source {
    Resources {
        urls: Vec<String>,
    },
    Chat {
        message_id: String,
        peer_id: String,
        file_ids: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "graphql", derive(plain_graphql::Enum))]
#[cfg_attr(
    feature = "graphql",
    graphql(name = "DownloadStatus", rename_items = "SCREAMING_SNAKE_CASE")
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Status {
    Pending,
    Downloading,
    Paused,
    Packing,
    Completed,
    Partial,
    Failed,
    Canceled,
}

impl Status {
    pub fn terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Partial | Self::Failed | Self::Canceled
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Failure {
    pub path: String,
    pub error: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Download {
    pub id: String,
    pub title: String,
    pub source: Source,
    pub target_dir: String,
    pub zip_name: Option<String>,
    pub status: Status,
    pub downloaded_bytes: i64,
    pub total_bytes: i64,
    pub bytes_per_second: i64,
    pub total_files: i32,
    pub done_files: i32,
    pub current_file: String,
    pub error: String,
    pub failures: Vec<Failure>,
}

#[derive(Clone, Copy)]
pub enum Command {
    Pause,
    Resume,
    Retry,
    Cancel,
    Remove,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct Attachment {
    pub id: String,
    pub uri: String,
    #[serde(rename = "fileName")]
    pub name: String,
    #[serde(rename = "size")]
    pub size_bytes: i64,
}
