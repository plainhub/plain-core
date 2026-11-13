use crate::graphql::resolvers::app::capability_types::Disk;
use crate::server::ServerState;
use plain_graphql::{Context, ID, Object, Result};

use plain_rs::features::services::settings::types::AppUpdate;
use plain_rs::infrastructure::host::bridge::Host;
use plain_rs::infrastructure::types::Long;
use std::sync::Arc;

#[derive(Default)]
pub struct PlatformQuery;

#[Object]
impl PlatformQuery {
    fn events(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
    ) -> Result<Vec<crate::graphql::resolvers::app::capability_types::Event>> {
        use crate::graphql::resolvers::app::capability_types::{Event, EventType};
        let state = ctx.data::<ServerState>()?;
        let needle = plain_libs::utils::search_dsl::parse(&query)
            .into_iter()
            .find(|field| field.name == "text")
            .map(|field| field.value)
            .unwrap_or_default();
        let rows = state.db.audit_events(
            offset.max(0) as usize,
            limit.max(0) as usize,
            (!needle.trim().is_empty()).then_some(needle.as_str()),
        )?;
        rows.into_iter()
            .filter_map(|row| {
                let kind = EventType::from_kind(&row.r#type)?;
                Some(
                    plain_libs::chrono::DateTime::parse_from_rfc3339(&row.created_at)
                        .map(|at| Event {
                            id: row.id.into(),
                            r#type: kind,
                            message: row.message,
                            client_id: row.client_id.map(ID),
                            created_at: plain_rs::infrastructure::types::Instant(
                                at.with_timezone(&plain_libs::chrono::Utc),
                            ),
                        })
                        .map_err(|error| plain_graphql::Error::new(error.to_string())),
                )
            })
            .collect()
    }

    async fn disks(&self, ctx: &Context<'_>) -> Result<Vec<Disk>> {
        require_capability(ctx, "DISK_MANAGER").await?;
        let disks = tokio::task::spawn_blocking(plain_rs::infrastructure::platform::disks::list)
            .await
            .map_err(|error| plain_graphql::Error::new(error.to_string()))?
            .map_err(plain_graphql::Error::new)?;
        Ok(disks
            .into_iter()
            .map(|disk| Disk {
                id: disk.id.into(),
                name: disk.name,
                path: disk.path,
                size_bytes: Long(disk.size_bytes),
                removable: disk.removable,
                model: disk.model,
            })
            .collect())
    }

    async fn app_update(&self, ctx: &Context<'_>) -> Result<AppUpdate> {
        Ok(
            plain_rs::features::services::settings::updates::snapshot(ctx.data::<ServerState>()?)
                .await?,
        )
    }
}

async fn require_capability(ctx: &Context<'_>, name: &str) -> Result<()> {
    plain_rs::features::services::capabilities::require(ctx.data::<Arc<Host>>()?, name)
        .await
        .map_err(|error| plain_graphql::Error::new(error.to_string()))
}

#[derive(Default)]
pub struct PlatformMutation;

#[Object]
impl PlatformMutation {
    async fn format_disk(&self, ctx: &Context<'_>, path: String) -> Result<bool> {
        require_capability(ctx, "DISK_MANAGER").await?;
        let state = ctx.data::<ServerState>()?;
        let request = plain_rs::infrastructure::host::commands::DiskFormatRequest {
            path,
            client_id: ctx.data_opt::<String>().cloned().unwrap_or_default(),
        };
        let value = state
            .host
            .call_wait(
                plain_rs::infrastructure::host::commands::SYSTEM_DISK_FORMAT,
                plain_libs::serde_json::to_value(request)?,
            )
            .await
            .map_err(plain_graphql::Error::new)?;
        if value != plain_libs::serde_json::Value::Bool(true) {
            return Err("Disk format failed".into());
        }
        Ok(true)
    }

    fn set_mount_alias(
        &self,
        ctx: &Context<'_>,
        id: plain_graphql::ID,
        alias: String,
    ) -> Result<bool> {
        let state = ctx.data::<ServerState>()?;
        plain_rs::features::prefs::storage::set_alias(&state.prefs, id.as_str(), &alias)?;
        Ok(true)
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/app/platform.rs"]
mod tests;
