mod directories;
mod discovery;
pub use directories::set_directories;
pub use discovery::{discover, discover_native};

use crate::features::files::index_scope;
use crate::infrastructure::db::IndexMode;
use crate::runtime::Runtime;

pub async fn set(
    state: &Runtime,
    id: &str,
    mode: IndexMode,
    included_dirs: Vec<String>,
    excluded_dirs: Vec<String>,
) -> Result<(), String> {
    state.native_client_id()?;
    if mode == IndexMode::Unconfigured {
        return Err("Select an index mode".into());
    }
    if plain_libs::serde_json::to_vec(&(&included_dirs, &excluded_dirs))
        .map_err(|error| error.to_string())?
        .len()
        > 65536
    {
        return Err("Index directory configuration is too large".into());
    }
    let volume = state
        .db
        .volumes()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|volume| volume.id == id)
        .ok_or("Volume not found")?;
    if volume.path.contains("://") {
        return Err("Filesystem volume required".into());
    }
    let included = directories::volume_directories(&volume, included_dirs)?;
    let excluded = directories::volume_directories(&volume, excluded_dirs)?;
    if !state
        .db
        .set_volume_index(id, mode, &included, &excluded)
        .map_err(|error| error.to_string())?
    {
        return Err("Volume not found".into());
    }
    #[cfg(feature = "runtime-media")]
    crate::features::services::files::catalog_scan::NativeMediaCatalog::new(state.clone())
        .refresh_scope(id)
        .await?;
    crate::features::services::files::catalog_events::publish(
        &state.events,
        [
            crate::features::enums::DataType::File,
            crate::features::enums::DataType::Image,
            crate::features::enums::DataType::Video,
            crate::features::enums::DataType::Audio,
            crate::features::enums::DataType::Doc,
        ]
        .into_iter()
        .map(|kind| kind.kind()),
    );
    Ok(())
}

pub fn defaults(state: &Runtime) -> Result<index_scope::Defaults, String> {
    #[cfg(feature = "runtime-media")]
    let mut defaults = state.media_scans.defaults()?;
    #[cfg(not(feature = "runtime-media"))]
    let mut defaults = index_scope::defaults();
    defaults.excluded_directories.extend([
        index_scope::normalized(&state.directory),
        index_scope::normalized(state.cache.root()),
    ]);
    #[cfg(feature = "runtime-media")]
    defaults
        .excluded_directories
        .extend(state.media_scans.policy()?.excluded_roots.iter().cloned());
    defaults.excluded_directories.sort();
    defaults.excluded_directories.dedup();
    Ok(defaults)
}
