use super::types::FileIndexScanProgress;
use crate::server::ServerState;
use plain_graphql::{Context, ID, Object, Result, SimpleObject};
use plain_rs::infrastructure::db::FileIndexMode;

use plain_rs::features::services::files::scanning;
use plain_rs::features::services::files::scanning::Command;

async fn state<'a>(ctx: &'a Context<'_>) -> Result<&'a ServerState> {
    let state = ctx.data::<ServerState>()?;
    crate::http::auth::public_gate::require(ctx, &state.prefs, &[super::public::STORAGE])?;
    if !crate::http::auth::public_gate::require_granted(ctx, &[super::public::STORAGE])
        .await
        .map_err(|error| plain_graphql::Error::new(error.to_string()))?
    {
        return Err("no_permission".into());
    }
    Ok(state)
}
async fn index_state<'a>(ctx: &'a Context<'_>) -> Result<&'a ServerState> {
    let state = state(ctx).await?;
    plain_rs::features::services::capabilities::require(&state.host, "MEDIA_SCAN")
        .await
        .map_err(|error| plain_graphql::Error::new(error.to_string()))?;
    Ok(state)
}
#[derive(Default)]
pub struct FileIndexScanQuery;
#[Object]
impl FileIndexScanQuery {
    async fn file_index_scan_progress(&self, ctx: &Context<'_>) -> Result<FileIndexScanProgress> {
        let progress = scanning::progress(state(ctx).await?).await?;
        Ok(FileIndexScanProgress {
            indexed: progress.indexed,
            pending: progress
                .total
                .map(|total| total.saturating_sub(progress.indexed)),
            total: progress.total,
            state: progress.state,
            error: progress.error,
        })
    }
    async fn file_index_defaults(&self, ctx: &Context<'_>) -> Result<FileIndexDefaults> {
        let defaults =
            plain_rs::features::services::files::indexing::defaults(index_state(ctx).await?)?;
        Ok(FileIndexDefaults {
            excluded_dirs: defaults.excluded_directories,
            excluded_names: defaults.excluded_names,
            excluded_packages: plain_rs::features::media::catalog_paths::MEDIA_PACKAGE_SUFFIXES
                .iter()
                .map(|suffix| suffix.to_string())
                .collect(),
            excluded_user_libraries: cfg!(target_os = "macos"),
        })
    }
    async fn file_index_volumes(&self, ctx: &Context<'_>) -> Result<Vec<FileIndexVolume>> {
        let state = index_state(ctx).await?;
        let volumes = plain_rs::features::services::files::indexing::discover(state).await?;
        Ok(volumes
            .into_iter()
            .filter(|volume| !volume.path.contains("://"))
            .map(|volume| FileIndexVolume {
                included_dirs: volume
                    .included_dirs
                    .iter()
                    .map(|dir| {
                        plain_rs::features::files::index_scope::absolute_directory(
                            &volume.path,
                            dir,
                        )
                    })
                    .collect(),
                excluded_dirs: volume
                    .excluded_dirs
                    .iter()
                    .map(|dir| {
                        plain_rs::features::files::index_scope::absolute_directory(
                            &volume.path,
                            dir,
                        )
                    })
                    .collect(),
                id: volume.id.into(),
                path: volume.path,
                is_available: volume.is_available,
                index_mode: volume.index_mode,
            })
            .collect())
    }
}
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
    async fn pause_file_index_scan(&self, ctx: &Context<'_>) -> Result<bool> {
        command(ctx, Command::Pause).await
    }
    async fn resume_file_index_scan(&self, ctx: &Context<'_>) -> Result<bool> {
        command(ctx, Command::Resume).await
    }
    async fn stop_file_index_scan(&self, ctx: &Context<'_>) -> Result<bool> {
        command(ctx, Command::Stop).await
    }
    async fn set_file_index_volume(
        &self,
        ctx: &Context<'_>,
        id: ID,
        index_mode: FileIndexMode,
        included_dirs: Vec<String>,
        excluded_dirs: Vec<String>,
    ) -> Result<bool> {
        plain_rs::features::services::files::indexing::set(
            index_state(ctx).await?,
            id.as_ref(),
            index_mode,
            included_dirs,
            excluded_dirs,
        )
        .await?;
        Ok(true)
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/media/scan.rs"]
mod tests;

#[derive(SimpleObject)]
pub struct FileIndexVolume {
    pub id: ID,
    pub path: String,
    pub is_available: bool,
    pub index_mode: FileIndexMode,
    pub included_dirs: Vec<String>,
    pub excluded_dirs: Vec<String>,
}

#[derive(SimpleObject)]
pub struct FileIndexDefaults {
    pub excluded_dirs: Vec<String>,
    pub excluded_names: Vec<String>,
    pub excluded_packages: Vec<String>,
    pub excluded_user_libraries: bool,
}
