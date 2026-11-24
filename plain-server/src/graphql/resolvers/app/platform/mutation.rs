use super::require_capability;
use crate::server::ServerState;
use plain_graphql::{Context, Object, Result};
use plain_rs::features::capability::Capability;

#[derive(Default)]
pub struct PlatformMutation;

#[Object]
impl PlatformMutation {
    async fn format_disk(&self, ctx: &Context<'_>, path: String) -> Result<bool> {
        require_capability(ctx, Capability::DiskManager).await?;
        Ok(plain_rs::features::services::disks::format(
            ctx.data::<ServerState>()?,
            ctx.data::<String>()?,
            path,
        )
        .await?)
    }

    async fn set_mount_alias(
        &self,
        ctx: &Context<'_>,
        id: plain_graphql::ID,
        alias: String,
    ) -> Result<bool> {
        let state = ctx.data::<ServerState>()?;
        state
            .file_platform
            .set_mount_alias(id.to_string(), alias)
            .await?;
        Ok(true)
    }
}
