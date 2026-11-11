use crate::graphql::types::parse_instant;
use plain_graphql::{Context, ID, Object, SimpleObject};

use plain_rs::features::files::shares::Root;
use plain_rs::features::files::shares::Service;
use plain_rs::infrastructure::db::Db;
use plain_rs::infrastructure::db::ShareRow;
use plain_rs::infrastructure::types::Instant;
use std::sync::Arc;

#[derive(SimpleObject)]
struct ShareRecord {
    id: ID,
    name: String,
    password: String,
    url_token: String,
    read_only: bool,
    expires_at: Option<Instant>,
    created_at: Instant,
    updated_at: Instant,
    roots: Vec<Root>,
}
fn record(row: ShareRow) -> plain_graphql::Result<ShareRecord> {
    let roots = Service::roots(&row).map_err(|e| plain_graphql::Error::new(e.to_string()))?;
    Ok(ShareRecord {
        id: ID(row.id),
        name: row.name,
        password: row.password,
        url_token: row.url_token,
        read_only: row.read_only,
        expires_at: row.expires_at.as_deref().map(parse_instant).transpose()?,
        created_at: parse_instant(&row.created_at)?,
        updated_at: parse_instant(&row.updated_at)?,
        roots,
    })
}
#[derive(Default)]
pub struct ShareHostQuery;
#[Object]
impl ShareHostQuery {
    fn share_record(
        &self,
        ctx: &Context<'_>,
        id: ID,
    ) -> plain_graphql::Result<Option<ShareRecord>> {
        ctx.data::<Arc<Db>>()?
            .share_get(&id)?
            .map(record)
            .transpose()
    }
    fn share_records(&self, ctx: &Context<'_>) -> plain_graphql::Result<Vec<ShareRecord>> {
        ctx.data::<Arc<Db>>()?
            .share_list()?
            .into_iter()
            .map(record)
            .collect()
    }
}
#[derive(Default)]
pub struct ShareHostMutation;
#[Object]
impl ShareHostMutation {
    async fn create_share(
        &self,
        ctx: &Context<'_>,
        name: String,
        real_paths: Vec<String>,
        read_only: bool,
        expires_at: Option<Instant>,
    ) -> plain_graphql::Result<ShareRecord> {
        let service = ctx.data::<Arc<Service>>()?.clone();
        record(
            blocking(move || {
                service.create(
                    name,
                    real_paths,
                    read_only,
                    expires_at.map(|i| i.0.to_rfc3339_opts(plain_libs::chrono::SecondsFormat::Millis, true)),
                )
            })
            .await?,
        )
    }
    async fn update_share(
        &self,
        ctx: &Context<'_>,
        id: ID,
        name: String,
        expires_at: Option<Instant>,
        real_paths: Option<Vec<String>>,
    ) -> plain_graphql::Result<ShareRecord> {
        let service = ctx.data::<Arc<Service>>()?.clone();
        record(
            blocking(move || {
                service.update(
                    &id,
                    &name,
                    expires_at.map(|i| i.0.to_rfc3339_opts(plain_libs::chrono::SecondsFormat::Millis, true)),
                    real_paths,
                )
            })
            .await?,
        )
    }
    fn delete_share(&self, ctx: &Context<'_>, id: ID) -> plain_graphql::Result<bool> {
        Ok(ctx.data::<Arc<Db>>()?.share_delete(&id)? > 0)
    }
}

async fn blocking<T: Send + 'static>(
    operation: impl FnOnce() -> plain_libs::anyhow::Result<T> + Send + 'static,
) -> plain_graphql::Result<T> {
    tokio::task::spawn_blocking(operation)
        .await?
        .map_err(|e| plain_graphql::Error::new(e.to_string()))
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/shares/host.rs"]
mod tests;
