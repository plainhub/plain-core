use crate::server::ContentServer;
use serde_json::Value;
use std::sync::Arc;

pub(super) fn handles(method: &str) -> bool {
    matches!(
        method,
        "systemRelaunchApp"
            | plain_rs::host::commands::SYSTEM_HOSTNAME_SET
            | plain_rs::host::commands::SYSTEM_DISK_FORMAT
            | plain_rs::host::commands::SYSTEM_SAMBA_SERVICE_FACTS
            | plain_rs::host::commands::SYSTEM_SAMBA_APPLY
            | plain_rs::host::commands::SYSTEM_SAMBA_PASSWORD
    )
}

pub(super) async fn dispatch(
    core: &Arc<ContentServer>,
    method: &str,
    params: Value,
) -> Result<Value, String> {
    match method {
        "systemRelaunchApp" => {
            tokio::task::spawn_blocking(plain_rs::platform::linux::install::service::restart)
                .await
                .map_err(|error| error.to_string())?
                .map_err(|error| error.to_string())?;
            Ok(Value::Bool(true))
        }
        plain_rs::host::commands::SYSTEM_HOSTNAME_SET => {
            let request: plain_rs::host::commands::HostnameRequest =
                serde_json::from_value(params).map_err(|error| error.to_string())?;
            tokio::task::spawn_blocking(move || {
                plain_rs::platform::linux::hostname::set(&request.name)
            })
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
            Ok(Value::Bool(true))
        }
        plain_rs::host::commands::SYSTEM_DISK_FORMAT => {
            let request: plain_rs::host::commands::DiskFormatRequest =
                serde_json::from_value(params).map_err(|error| error.to_string())?;
            let core = core.clone();
            tokio::task::spawn_blocking(move || {
                let store = core
                    .headless_state()
                    .store
                    .get()
                    .map_err(|e| e.to_string())?;
                let events = crate::headless::storage::event_sink(&core, false)?;
                let db = core.headless_state().db.clone();
                let client_id =
                    (!request.client_id.is_empty()).then_some(request.client_id.as_str());
                let result = plain_rs::platform::linux::disks::format::format_disk_single_partition(
                    &store,
                    &request.path,
                    |mount| {
                        let _ = db.audit_event_add("unmount", mount, client_id);
                    },
                    &events,
                );
                let _ = db.audit_event_add(
                    if result.is_ok() {
                        "format_disk"
                    } else {
                        "format_disk_failed"
                    },
                    &request.path,
                    client_id,
                );
                result
                    .map(|_| Value::Bool(true))
                    .map_err(|error| error.to_string())
            })
            .await
            .map_err(|error| error.to_string())?
        }
        plain_rs::host::commands::SYSTEM_SAMBA_SERVICE_FACTS => {
            let facts = tokio::task::spawn_blocking(|| {
                let status = plain_rs::platform::linux::samba::get_service_status();
                plain_rs::host::commands::SambaServiceFacts {
                    name: status.name,
                    active: status.active,
                    enabled: status.enabled,
                }
            })
            .await
            .map_err(|error| error.to_string())?;
            serde_json::to_value(facts).map_err(|error| error.to_string())
        }
        plain_rs::host::commands::SYSTEM_SAMBA_APPLY => {
            let request: plain_rs::host::commands::SambaApplyRequest =
                serde_json::from_value(params).map_err(|error| error.to_string())?;
            let core = core.clone();
            tokio::task::spawn_blocking(move || {
                plain_rs::platform::linux::samba::apply(core.preferences()?, &request.settings, "")
                    .map(|_| Value::Bool(true))
                    .map_err(|error| error.to_string())
            })
            .await
            .map_err(|error| error.to_string())?
        }
        plain_rs::host::commands::SYSTEM_SAMBA_PASSWORD => {
            let request: plain_rs::host::commands::SambaPasswordRequest =
                serde_json::from_value(params).map_err(|error| error.to_string())?;
            let core = core.clone();
            tokio::task::spawn_blocking(move || {
                core.preferences()?;
                plain_rs::platform::linux::samba::set_password(&request.password)
                    .map(|_| Value::Bool(true))
                    .map_err(|error| error.to_string())
            })
            .await
            .map_err(|error| error.to_string())?
        }
        _ => Err(format!("Unsupported Linux host method: {method}")),
    }
}
