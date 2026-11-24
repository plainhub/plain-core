use super::require_capability;
use crate::graphql::resolvers::app::capability_types::Disk;
use crate::server::ServerState;
use plain_graphql::{Context, ID, Object, Result};
use plain_rs::features::capability::Capability;
use plain_rs::features::services::settings::types::AppUpdate;
use plain_rs::infrastructure::types::Long;

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
        use crate::graphql::resolvers::app::capability_types::Event;
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
            .map(|row| {
                plain_libs::chrono::DateTime::parse_from_rfc3339(&row.created_at)
                    .map(|at| Event {
                        id: row.id.into(),
                        r#type: row.r#type,
                        data: row.data,
                        client_id: row.client_id.map(ID),
                        client_name: row.client_name,
                        client_ip: row.client_ip,
                        os_name: row.os_name,
                        os_version: row.os_version,
                        app_name: row.app_name,
                        app_version: row.app_version,
                        created_at: plain_rs::infrastructure::types::Instant(
                            at.with_timezone(&plain_libs::chrono::Utc),
                        ),
                    })
                    .map_err(|error| plain_graphql::Error::from(error))
            })
            .collect()
    }

    async fn disks(&self, ctx: &Context<'_>) -> Result<Vec<Disk>> {
        require_capability(ctx, Capability::DiskManager).await?;
        let disks = tokio::task::spawn_blocking(plain_rs::infrastructure::platform::disks::list)
            .await
            .map_err(|error| plain_graphql::Error::from(error))?
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
