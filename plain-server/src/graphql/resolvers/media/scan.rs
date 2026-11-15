use super::types::FileIndexScanProgress;
use crate::server::ServerState;
use plain_graphql::{Context, ID, InputObject, Object, Result, SimpleObject};
use plain_rs::infrastructure::db::{FileIndexMode, FileIndexScanStatus};
use plain_rs::infrastructure::types::{Instant, Long};

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
    async fn file_index_scan_progress(
        &self,
        ctx: &Context<'_>,
        volume_id: Option<ID>,
    ) -> Result<FileIndexScanProgress> {
        let progress =
            scanning::progress(state(ctx).await?, volume_id.as_ref().map(|id| id.as_str())).await?;
        Ok(FileIndexScanProgress {
            volume_id: progress.volume_id.map(ID::from),
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
        volumes
            .into_iter()
            .filter(|volume| !volume.path.contains("://"))
            .map(|volume| -> Result<FileIndexVolume> {
                Ok(FileIndexVolume {
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
                    last_scan_at: volume
                        .last_scan_at
                        .as_deref()
                        .map(crate::graphql::types::parse_instant)
                        .transpose()?,
                    last_scan_status: volume.last_scan_status,
                    last_scan_count: volume.last_scan_count.map(Long),
                    last_scan_total: volume.last_scan_total.map(Long),
                })
            })
            .collect()
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
        input: FileIndexVolumeInput,
    ) -> Result<bool> {
        plain_rs::features::services::files::indexing::set(
            index_state(ctx).await?,
            input.volume_id.as_ref(),
            input.index_mode,
            input.included_dirs,
            input.excluded_dirs,
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
    pub last_scan_at: Option<Instant>,
    pub last_scan_status: Option<FileIndexScanStatus>,
    pub last_scan_count: Option<Long>,
    pub last_scan_total: Option<Long>,
}

#[derive(SimpleObject)]
pub struct FileIndexDefaults {
    pub excluded_dirs: Vec<String>,
    pub excluded_names: Vec<String>,
    pub excluded_packages: Vec<String>,
    pub excluded_user_libraries: bool,
}

#[derive(InputObject)]
pub struct FileIndexVolumeInput {
    pub volume_id: ID,
    pub index_mode: FileIndexMode,
    pub included_dirs: Vec<String>,
    pub excluded_dirs: Vec<String>,
}
