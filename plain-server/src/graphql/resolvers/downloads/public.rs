use crate::server::ServerState;
use plain_graphql::{Context, Object};

use plain_rs::features::chat::attachments::download_queue::Queue;
use std::sync::Arc;

async fn run(
    ctx: &Context<'_>,
    work: impl FnOnce(Arc<Queue>) -> plain_libs::anyhow::Result<bool> + Send + 'static,
) -> plain_graphql::Result<bool> {
    let queue = ctx.data::<ServerState>()?.downloads.clone();
    tokio::task::spawn_blocking(move || work(queue))
        .await?
        .map_err(|error| plain_graphql::Error::new(error.to_string()))
}

#[derive(Default)]
pub struct DownloadMutation;

#[Object]
impl DownloadMutation {
    async fn pause_download(
        &self,
        ctx: &Context<'_>,
        message_id: plain_graphql::ID,
    ) -> plain_graphql::Result<bool> {
        run(ctx, move |queue| {
            queue.control_message(&message_id, None, "pause")
        })
        .await
    }
    async fn download_peer_file(
        &self,
        ctx: &Context<'_>,
        message_id: plain_graphql::ID,
        peer_id: plain_graphql::ID,
    ) -> plain_graphql::Result<bool> {
        run(ctx, move |queue| {
            queue.enqueue_message(&message_id, &peer_id)
        })
        .await
    }
    async fn resume_download(
        &self,
        ctx: &Context<'_>,
        message_id: plain_graphql::ID,
        peer_id: plain_graphql::ID,
    ) -> plain_graphql::Result<bool> {
        run(ctx, move |queue| {
            if queue.control_message(&message_id, Some(&peer_id), "resume")? {
                Ok(true)
            } else {
                queue.enqueue_message(&message_id, &peer_id)
            }
        })
        .await
    }
    async fn retry_download(
        &self,
        ctx: &Context<'_>,
        message_id: plain_graphql::ID,
        peer_id: plain_graphql::ID,
    ) -> plain_graphql::Result<bool> {
        run(ctx, move |queue| queue.retry_message(&message_id, &peer_id)).await
    }
}
