use plain_libs::anyhow::Result;
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
        .map_err(plain_libs::anyhow::Error::msg)
}

#[cfg(target_os = "linux")]
pub(crate) fn event_sink(
    core: &plain_server::server::ContentServer,
    client: Option<plain_rs::infrastructure::db::AuditEventClient>,
) -> Result<plain_rs::infrastructure::platform::linux::mounts::automount::EventSink, String> {
    let catalog = core.native_media_catalog()?;
    let state = core.runtime().clone();
    Ok(std::sync::Arc::new(move |event| {
        use plain_rs::infrastructure::platform::linux::mounts::automount::MountEvent;
        if let MountEvent::Audit(event) = event {
            state
                .audit_events
                .record(event, client.clone())
                .map_err(plain_libs::anyhow::Error::msg)?;
        } else {
            if state
                .peer_status
                .public_active
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                let result = (|| -> Result<(), String> {
                    plain_rs::features::services::files::indexing::discover_native(&state)?;
                    catalog.configure_policy((*state.media_scans.policy()?).clone())?;
                    catalog.watch_source_directories()?;
                    Ok(())
                })();
                if let Err(error) = result {
                    log::error!("mount sources update failed: {error}");
                }
            }
        }
        Ok(())
    }))
}

#[cfg(test)]
#[path = "../../tests/plain-cli/unit/storage.rs"]
mod tests;
