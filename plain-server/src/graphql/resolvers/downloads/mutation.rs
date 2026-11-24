use super::error;
use super::inputs::DownloadInput;
use super::models::Download;
use crate::server::ServerState;
use plain_graphql::{Context, ID, Object};
use plain_rs::features::downloads::Command;
use plain_rs::features::services::downloads;

#[derive(Default)]
pub struct DownloadMutation;

#[Object]
impl DownloadMutation {
    async fn create_download(
        &self,
        ctx: &Context<'_>,
        input: DownloadInput,
    ) -> plain_graphql::Result<Download> {
        let (source, target, zip_name) = input.source()?;
        downloads::create(ctx.data::<ServerState>()?, source, target, zip_name)
            .await
            .map(Into::into)
            .map_err(error)
    }
    async fn pause_download(&self, ctx: &Context<'_>, id: ID) -> plain_graphql::Result<bool> {
        control(ctx, &id, Command::Pause).await
    }
    async fn resume_download(&self, ctx: &Context<'_>, id: ID) -> plain_graphql::Result<bool> {
        control(ctx, &id, Command::Resume).await
    }
    async fn retry_download(&self, ctx: &Context<'_>, id: ID) -> plain_graphql::Result<bool> {
        control(ctx, &id, Command::Retry).await
    }
    async fn cancel_download(&self, ctx: &Context<'_>, id: ID) -> plain_graphql::Result<bool> {
        control(ctx, &id, Command::Cancel).await
    }
    async fn remove_download(&self, ctx: &Context<'_>, id: ID) -> plain_graphql::Result<bool> {
        control(ctx, &id, Command::Remove).await
    }
}

async fn control(ctx: &Context<'_>, id: &str, command: Command) -> plain_graphql::Result<bool> {
    let state = ctx.data::<ServerState>()?;
    state
        .download_runtime
        .control(state, id, command)
        .await
        .map_err(error)
}
