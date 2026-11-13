use super::{inputs::DownloadInput, models::Download};
use crate::server::ServerState;
use plain_graphql::{Context, ID, Object};
use plain_rs::features::downloads::Command;
use plain_rs::features::services::downloads;

fn error(error: plain_libs::anyhow::Error) -> plain_graphql::Error {
    plain_graphql::Error::new(error.to_string())
}

#[derive(Default)]
pub struct DownloadQuery;
#[Object]
impl DownloadQuery {
    fn downloads(&self, ctx: &Context<'_>) -> plain_graphql::Result<Vec<Download>> {
        Ok(ctx
            .data::<ServerState>()?
            .downloads
            .snapshot()
            .into_iter()
            .map(Into::into)
            .collect())
    }
}
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

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/downloads/public.rs"]
mod tests;
