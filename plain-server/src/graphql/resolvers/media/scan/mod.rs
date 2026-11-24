pub mod mutation;
pub mod query;

use crate::server::ServerState;
use plain_graphql::{Context, ID, InputObject, Result, SimpleObject};
use plain_rs::features::access_error::AccessErrorCode;
use plain_rs::features::capability::Capability;
use plain_rs::infrastructure::db::{FileIndexMode, FileIndexScanStatus};
use plain_rs::infrastructure::types::{Instant, Long};

async fn state<'a>(ctx: &'a Context<'_>) -> Result<&'a ServerState> {
    let state = ctx.data::<ServerState>()?;
    crate::http::auth::public_gate::require(ctx, &state.prefs, &[super::query::STORAGE])?;
    if !crate::http::auth::public_gate::require_granted(ctx, &[super::query::STORAGE])
        .await
        .map_err(|error| plain_graphql::Error::from(error))?
    {
        return Err(AccessErrorCode::NoPermission.into());
    }
    Ok(state)
}

async fn index_state<'a>(ctx: &'a Context<'_>) -> Result<&'a ServerState> {
    let state = state(ctx).await?;
    plain_rs::features::services::capabilities::require(&state.host, Capability::MediaScan)
        .await
        .map_err(|error| plain_graphql::Error::from(error))?;
    Ok(state)
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-server/unit/graphql/resolvers/media/scan/mod.rs"]
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
    /// Directory rules relative to every direct child of profilesDir.
    pub excluded_profile_dirs: Vec<FileIndexProfileDir>,
    /// Case-insensitive directory suffixes excluded from media recognition only.
    pub media_excluded_dir_suffixes: Vec<String>,
}

#[derive(SimpleObject)]
pub struct FileIndexProfileDir {
    /// Absolute parent directory ending in a slash; rules apply to every direct child profile.
    pub profiles_dir: String,
    /// Excluded directory relative to each profile.
    pub excluded_dir: String,
    /// Profile-relative exceptions; their ancestors remain traversable and other rules still apply.
    pub except_dirs: Vec<String>,
}

#[derive(InputObject)]
pub struct FileIndexVolumeInput {
    pub index_mode: FileIndexMode,
    pub included_dirs: Vec<String>,
    pub excluded_dirs: Vec<String>,
}
