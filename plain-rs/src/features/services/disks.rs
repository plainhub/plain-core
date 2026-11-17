use crate::features::services::disk_format_error::DiskFormatErrorCode;
use crate::features::audit_events::{
    AuditEvent, DiskFormatEventData, DiskFormatFailedEventData, EventDisk, DiskFormatFailure,
};
use crate::infrastructure::host::commands::{DiskFormatRequest, SYSTEM_DISK_FORMAT};
use crate::infrastructure::types::Long;
use crate::runtime::Runtime;
use plain_libs::anyhow::{Result, anyhow};
use plain_libs::serde_json::Value;

pub async fn format(state: &Runtime, client_id: &str, path: String) -> Result<bool> {
    let actor = super::audit_events::client(state, client_id)?;
    let mut disk = EventDisk {
        id: None,
        name: None,
        path: path.clone(),
        size_bytes: None,
    };
    let result = async {
        let disks = tokio::task::spawn_blocking(crate::infrastructure::platform::disks::list)
            .await?
            .map_err(plain_libs::anyhow::Error::msg)?;
        if let Some(found) = disks.into_iter().find(|disk| disk.path == path) {
            disk = EventDisk {
                id: Some(found.id),
                name: Some(found.name),
                path: found.path,
                size_bytes: Some(Long(found.size_bytes)),
            };
        }
        let request = DiskFormatRequest {
            path,
            client_id: client_id.to_owned(),
        };
        let value = state
            .host
            .call_wait(
                SYSTEM_DISK_FORMAT,
                plain_libs::serde_json::to_value(request)?,
            )
            .await
            .map_err(plain_libs::anyhow::Error::msg)?;
        if value == Value::Bool(true) {
            Ok(true)
        } else {
            Err(anyhow!("Disk format failed"))
        }
    }
    .await;
    let event = match &result {
        Ok(_) => AuditEvent::FormatDisk(DiskFormatEventData {
            disk,
            file_system: "ext4".into(),
        }),
        Err(_) => AuditEvent::FormatDiskFailed(DiskFormatFailedEventData {
            disk,
            file_system: "ext4".into(),
            failure: DiskFormatFailure {
                reason: DiskFormatErrorCode::FormatDiskFailed,
                detail: None,
            },
        }),
    };
    if let Err(error) = state.audit_events.record(event, Some(actor)) {
        return Err(match result {
            Ok(_) => anyhow!("Disk formatted, but event submission failed: {error}"),
            Err(operation) => anyhow!("{operation}; event submission failed: {error}"),
        });
    }
    result
}
