use crate::infrastructure::db::{Db, VolumeRow};
use crate::runtime::Runtime;
use std::path::Path;

pub async fn discover(state: &Runtime) -> Result<Vec<VolumeRow>, String> {
    state.native_client_id()?;
    let state = state.clone();
    if !state.file_platform.uses_sdk() {
        return tokio::task::spawn_blocking(move || discover_native(&state))
            .await
            .map_err(|error| error.to_string())?;
    }
    let facts = state
        .file_platform
        .mounts(state.prefs.clone())
        .await
        .map_err(|error| error.to_string())?;
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Mount {
        path: String,
        drive_type: String,
    }
    let mounts: Vec<Mount> = serde_json::from_value(facts).map_err(|error| error.to_string())?;
    tokio::task::spawn_blocking(move || {
        let paths = mounts
            .into_iter()
            .filter(|mount| mount.drive_type != "APP")
            .map(|mount| mount.path);
        register_mounts(&state.db, paths)
    })
    .await
    .map_err(|error| error.to_string())?
}

pub fn discover_native(state: &Runtime) -> Result<Vec<VolumeRow>, String> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Mount {
        mount_point: Option<String>,
    }
    let facts = crate::features::services::files::mounts::native_facts(&state.prefs)?;
    let mounts: Vec<Mount> = serde_json::from_value(facts).map_err(|error| error.to_string())?;
    register_mounts(
        &state.db,
        mounts.into_iter().filter_map(|mount| mount.mount_point),
    )
}

fn register_mounts(db: &Db, paths: impl IntoIterator<Item = String>) -> Result<Vec<VolumeRow>, String> {
    for path in paths.into_iter().filter(|path| !path.is_empty()) {
        db.register_volume(Path::new(&path))
            .map_err(|error| error.to_string())?;
    }
    db.refresh_volume_availability()
        .map_err(|error| error.to_string())?;
    db.volumes().map_err(|error| error.to_string())
}
