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
    tokio::task::spawn_blocking(move || register_mounts(&state.db, facts))
        .await
        .map_err(|error| error.to_string())?
}

pub fn discover_native(state: &Runtime) -> Result<Vec<VolumeRow>, String> {
    let facts = crate::features::services::files::mounts::native_facts(&state.prefs)?;
    register_mounts(&state.db, facts)
}

fn register_mounts(db: &Db, facts: serde_json::Value) -> Result<Vec<VolumeRow>, String> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Mount {
        mount_point: String,
    }
    let mounts: Vec<Mount> = serde_json::from_value(facts).map_err(|error| error.to_string())?;
    for mount in mounts {
        if !mount.mount_point.is_empty() {
            db.register_volume(Path::new(&mount.mount_point))
                .map_err(|error| error.to_string())?;
        }
    }
    db.refresh_volume_availability()
        .map_err(|error| error.to_string())?;
    db.volumes().map_err(|error| error.to_string())
}
