use plain_libs::serde_json::Value;
use plain_server::server::ContentServer;
use std::sync::Arc;

pub(super) fn handles(method: &str) -> bool {
    matches!(
        method,
        "systemRelaunchApp" | plain_rs::infrastructure::host::commands::SYSTEM_DISK_FORMAT
    )
}

pub(super) async fn dispatch(
    core: &Arc<ContentServer>,
    method: &str,
    params: Value,
) -> Result<Value, String> {
    match method {
        "systemRelaunchApp" => {
            tokio::task::spawn_blocking(
                plain_rs::infrastructure::platform::linux::install::service::restart,
            )
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
            Ok(Value::Bool(true))
        }
        plain_rs::infrastructure::host::commands::SYSTEM_DISK_FORMAT => {
            let request: plain_rs::infrastructure::host::commands::DiskFormatRequest =
                plain_libs::serde_json::from_value(params).map_err(|error| error.to_string())?;
            let core = core.clone();
            tokio::task::spawn_blocking(move || {
                let store = core.runtime().store.get().map_err(|e| e.to_string())?;
                let client = plain_rs::features::services::audit_events::client(core.runtime(), &request.client_id)
                    .map_err(|error| error.to_string())?;
                let events = crate::storage::event_sink(&core, Some(client))?;
                let result =
                    plain_rs::infrastructure::platform::linux::disks::format::format_disk_single_partition(
                        &store,
                        &request.path,
                        &events,
                    );
                result
                    .map(|_| Value::Bool(true))
                    .map_err(|error| error.to_string())
            })
            .await
            .map_err(|error| error.to_string())?
        }
        _ => Err(format!("Unsupported Linux host method: {method}")),
    }
}
