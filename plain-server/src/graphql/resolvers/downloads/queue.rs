use crate::graphql::resolvers::peers::public::Peer;
use crate::server::ServerState;
use async_graphql::{Context, Object, SimpleObject};
use plain_rs::chat::attachments::download_status::DownloadStatus;
use plain_rs::types::Long;
use serde::Deserialize;

#[derive(SimpleObject)]
struct ChatDownloadFile {
    file_id: String,
    uri: String,
    size: Long,
    duration_ms: Long,
    width: i32,
    height: i32,
    summary: String,
    file_name: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileReceipt {
    id: String,
    uri: String,
    size: Long,
    duration_ms: Option<i64>,
    #[serde(alias = "legacyDurationSec")]
    duration: Option<i64>,
    #[serde(default)]
    width: i32,
    #[serde(default)]
    height: i32,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    file_name: String,
}
impl TryFrom<FileReceipt> for ChatDownloadFile {
    type Error = async_graphql::Error;
    fn try_from(file: FileReceipt) -> Result<Self, Self::Error> {
        let duration = match file.duration_ms {
            Some(value) => value,
            None => file
                .duration
                .unwrap_or_default()
                .checked_mul(1000)
                .ok_or_else(|| async_graphql::Error::new("Attachment duration overflow"))?,
        };
        Ok(Self {
            file_id: file.id,
            uri: file.uri,
            size: file.size,
            duration_ms: Long(duration),
            width: file.width,
            height: file.height,
            summary: file.summary,
            file_name: file.file_name,
        })
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskReceipt {
    id: String,
    message_id: async_graphql::ID,
    file: FileReceipt,
    peer: plain_rs::db::DPeer,
    status: DownloadStatus,
    error: String,
    downloaded: Long,
    total: Long,
    speed: Long,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueueReceipt {
    runtime_id: String,
    revision: Long,
    tasks: Vec<TaskReceipt>,
}
#[derive(SimpleObject)]
struct ChatDownloadTask {
    id: async_graphql::ID,
    message_id: async_graphql::ID,
    file: ChatDownloadFile,
    peer: Peer,
    status: DownloadStatus,
    error: String,
    downloaded: Long,
    total: Long,
    speed: Long,
}
#[derive(SimpleObject)]
struct ChatDownloadQueue {
    runtime_id: String,
    revision: Long,
    tasks: Vec<ChatDownloadTask>,
}
fn snapshot(
    value: serde_json::Value,
    online: &std::collections::HashSet<&str>,
) -> async_graphql::Result<ChatDownloadQueue> {
    let local = plain_rs::transport::lan::local_interfaces();
    let receipt: QueueReceipt = serde_json::from_value(value)?;
    Ok(ChatDownloadQueue {
        runtime_id: receipt.runtime_id,
        revision: receipt.revision,
        tasks: receipt
            .tasks
            .into_iter()
            .map(|task| {
                Ok(ChatDownloadTask {
                    id: task.id.into(),
                    message_id: task.message_id.into(),
                    file: task.file.try_into()?,
                    peer: {
                        let connected = online.contains(task.peer.id.as_str());
                        Peer::from_record(task.peer, connected, &local)
                    },
                    status: task.status,
                    error: task.error,
                    downloaded: task.downloaded,
                    total: task.total,
                    speed: task.speed,
                })
            })
            .collect::<async_graphql::Result<Vec<_>>>()?,
    })
}
#[derive(Default)]
pub struct ChatDownloadQuery;
#[Object]
impl ChatDownloadQuery {
    async fn chat_download_queue(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<ChatDownloadQueue> {
        let state = ctx.data::<ServerState>()?;
        let queue = state.downloads.clone();
        let connections = state.peer_status.connections.snapshot(&state.db);
        let online = connections["online"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .collect();
        snapshot(
            tokio::task::spawn_blocking(move || queue.snapshot()).await?,
            &online,
        )
    }
}
#[derive(Default)]
pub struct ChatDownloadMutation;
#[Object]
impl ChatDownloadMutation {
    async fn enqueue_chat_download(
        &self,
        ctx: &Context<'_>,
        message_id: async_graphql::ID,
        file_id: String,
        peer_id: async_graphql::ID,
    ) -> async_graphql::Result<bool> {
        let queue = ctx.data::<ServerState>()?.downloads.clone();
        tokio::task::spawn_blocking(move || queue.enqueue(&message_id, &file_id, &peer_id))
            .await?
            .map_err(|error| async_graphql::Error::new(error.to_string()))
    }
    async fn pause_chat_download(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::ID,
    ) -> async_graphql::Result<bool> {
        let queue = ctx.data::<ServerState>()?.downloads.clone();
        tokio::task::spawn_blocking(move || queue.control(&id, "pause"))
            .await?
            .map_err(|error| async_graphql::Error::new(error.to_string()))
    }
    async fn resume_chat_download(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::ID,
    ) -> async_graphql::Result<bool> {
        let queue = ctx.data::<ServerState>()?.downloads.clone();
        tokio::task::spawn_blocking(move || queue.control(&id, "resume"))
            .await?
            .map_err(|error| async_graphql::Error::new(error.to_string()))
    }
    async fn retry_chat_download(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::ID,
    ) -> async_graphql::Result<bool> {
        let queue = ctx.data::<ServerState>()?.downloads.clone();
        tokio::task::spawn_blocking(move || queue.control(&id, "retry"))
            .await?
            .map_err(|error| async_graphql::Error::new(error.to_string()))
    }
    async fn remove_chat_download(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::ID,
    ) -> async_graphql::Result<bool> {
        let queue = ctx.data::<ServerState>()?.downloads.clone();
        tokio::task::spawn_blocking(move || queue.control(&id, "remove"))
            .await?
            .map_err(|error| async_graphql::Error::new(error.to_string()))
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/graphql/resolvers/downloads/queue.rs"]
mod tests;
