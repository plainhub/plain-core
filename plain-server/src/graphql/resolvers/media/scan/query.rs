use super::super::types::FileIndexScanProgress;
use super::{FileIndexDefaults, FileIndexProfileDir, FileIndexVolume, index_state, state};
use plain_graphql::{Context, ID, Object, Result};
use plain_rs::features::services::files::scanning;
use plain_rs::infrastructure::types::Long;

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
            excluded_profile_dirs: plain_rs::features::files::index_scope::platform::PROFILE_DIRS
                .iter()
                .map(|rule| FileIndexProfileDir {
                    profiles_dir: rule.profiles_dir.to_owned(),
                    excluded_dir: rule.excluded_dir.to_owned(),
                    except_dirs: rule
                        .except_dirs
                        .iter()
                        .map(|dir| (*dir).to_owned())
                        .collect(),
                })
                .collect(),
            media_excluded_dir_suffixes:
                plain_rs::features::files::index_scope::platform::MEDIA_DIR_SUFFIXES
                    .iter()
                    .map(|suffix| (*suffix).to_owned())
                    .collect(),
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
