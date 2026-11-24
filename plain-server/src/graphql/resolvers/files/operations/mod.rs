pub mod mutation;
pub mod query;

use crate::graphql::types::facts::text;
use plain_graphql::{Context, Enum, SimpleObject};
use plain_libs::serde_json::Value;
use plain_rs::features::access::AccessFeature;
use plain_rs::infrastructure::types::Long;
use std::sync::Arc;

const STORAGE: AccessFeature = AccessFeature::Files;

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, Default)]
pub enum MergeTaskStatus {
    #[default]
    None,
    Started,
    Merging,
    Done,
    Failed,
}

impl MergeTaskStatus {
    fn parse(value: &str) -> Self {
        match value {
            "STARTED" => Self::Started,
            "MERGING" => Self::Merging,
            "DONE" => Self::Done,
            "FAILED" => Self::Failed,
            _ => Self::None,
        }
    }
}

#[derive(SimpleObject, Clone, Debug, Default)]
pub struct MergeTask {
    pub status: MergeTaskStatus,
    pub value: Option<String>,
    #[graphql(name = "mergedSize")]
    pub merged_size: Option<Long>,
    pub error: Option<String>,
}

#[derive(SimpleObject)]
pub struct UploadedChunk {
    pub index: i32,
    pub size_bytes: Long,
}

fn merge_task(value: &Value) -> MergeTask {
    MergeTask {
        status: MergeTaskStatus::parse(&text(value, "status")),
        value: value["value"].as_str().map(str::to_string),
        merged_size: value["mergedSize"].as_i64().map(Long),
        error: value["error"].as_str().map(str::to_string),
    }
}

fn prefs<'a>(ctx: &'a Context<'_>) -> &'a Arc<plain_rs::features::prefs::Prefs> {
    ctx.data_unchecked::<Arc<plain_rs::features::prefs::Prefs>>()
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-server/unit/graphql/resolvers/files/operations/mod.rs"]
mod tests;
