use anyhow::Result;
use std::path::PathBuf;

pub(super) fn validate_automount(enabled: bool) -> Result<()> {
    if !enabled {
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    return plain_rs::libs::platform::linux::install::service::require_root();
    #[cfg(not(target_os = "linux"))]
    anyhow::bail!("Automatic disk mounting is available on Linux");
}

pub(super) fn source_directories(
    prefs: &plain_rs::features::prefs::Prefs,
    requested: &[PathBuf],
) -> Result<Vec<String>> {
    if !requested.is_empty() {
        let directories = plain_rs::features::prefs::file_index::validate_directories(
            requested
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
        )
        .map_err(anyhow::Error::msg)?;
        plain_rs::features::prefs::media::set_source_directories(prefs, &directories)?;
    }
    plain_rs::features::prefs::file_index::source_directories(
        prefs,
        &plain_rs::features::prefs::file_index::defaults(),
    )
    .map_err(anyhow::Error::msg)
}

pub(super) fn start_source_scan(
    catalog: &plain_rs::features::services::files::catalog_scan::NativeMediaCatalog,
) -> Result<plain_rs::features::services::files::media_scans::Snapshot, String> {
    catalog.start_source_scan("/")
}

#[cfg(target_os = "linux")]
pub(crate) fn event_sink(
    core: &plain_server::server::ContentServer,
) -> Result<plain_rs::libs::platform::linux::mounts::automount::EventSink, String> {
    let db = core.runtime().db.clone();
    let catalog = core.native_media_catalog()?;
    let state = core.runtime().clone();
    Ok(std::sync::Arc::new(move |kind, message| {
        if kind == "mounts_changed" {
            if state
                .peer_status
                .public_active
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                let result = (|| -> Result<(), String> {
                    catalog.watch_source_directories()?;
                    if !catalog.source_directories()?.is_empty() {
                        start_source_scan(&catalog)?;
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    log::error!("mount sources update failed: {error}");
                }
            }
            return;
        }
        if let Err(error) = db.audit_event_add(kind, message, None) {
            log::error!("mount audit failed: {error}");
        }
    }))
}

#[cfg(test)]
#[path = "../../tests/plain-cli/unit/storage.rs"]
mod tests;
