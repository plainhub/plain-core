use super::{FileIndexVolumeInput, index_state, state};
use plain_graphql::{Context, ID, Object, Result};
use plain_rs::features::services::files::scanning;
use plain_rs::features::services::files::scanning::Command;

async fn command(ctx: &Context<'_>, action: Command) -> Result<bool> {
    scanning::execute(state(ctx).await?, action).await?;
    Ok(true)
}

#[derive(Default)]
pub struct FileIndexScanMutation;

#[Object]
impl FileIndexScanMutation {
    async fn rebuild_file_index(&self, ctx: &Context<'_>, volume_id: Option<ID>) -> Result<bool> {
        command(ctx, Command::Rebuild(volume_id.map(String::from))).await
    }
    async fn pause_file_index(&self, ctx: &Context<'_>, volume_id: Option<ID>) -> Result<bool> {
        command(ctx, Command::Pause(volume_id.map(String::from))).await
    }
    async fn resume_file_index(&self, ctx: &Context<'_>, volume_id: Option<ID>) -> Result<bool> {
        command(ctx, Command::Resume(volume_id.map(String::from))).await
    }
    async fn stop_file_index(&self, ctx: &Context<'_>, volume_id: Option<ID>) -> Result<bool> {
        command(ctx, Command::Stop(volume_id.map(String::from))).await
    }
    async fn set_file_index_volume(
        &self,
        ctx: &Context<'_>,
        volume_id: ID,
        input: FileIndexVolumeInput,
    ) -> Result<bool> {
        plain_rs::features::services::files::indexing::set(
            index_state(ctx).await?,
            volume_id.as_ref(),
            input.index_mode,
            input.included_dirs,
            input.excluded_dirs,
        )
        .await?;
        Ok(true)
    }
}
