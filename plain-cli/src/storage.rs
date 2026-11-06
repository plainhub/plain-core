use anyhow::Result;
use std::path::PathBuf;

pub(super) fn configure_index(
    db: &plain_rs::infrastructure::db::Db,
    requested: &[PathBuf],
) -> Result<()> {
    let directories = requested
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    plain_rs::features::services::files::indexing::set_directories(db, &directories)
        .map_err(anyhow::Error::msg)
}

#[cfg(target_os = "linux")]
pub(crate) fn event_sink(
    core: &plain_server::server::ContentServer,
) -> Result<plain_rs::infrastructure::platform::linux::mounts::automount::EventSink, String> {
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
                    plain_rs::features::services::files::indexing::discover_native(&state)?;
                    catalog.configure_policy((*state.media_scans.policy()?).clone())?;
                    catalog.watch_source_directories()?;
                    if !catalog.source_directories()?.is_empty() {
                        catalog.start_source_scan("/")?;
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
