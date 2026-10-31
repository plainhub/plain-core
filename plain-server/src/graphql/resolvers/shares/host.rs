use crate::graphql::types::parse_instant;
use async_graphql::{Context, ID, Object, SimpleObject};
use plain_rs::db::Db;
use plain_rs::db::ShareRow;
use plain_rs::files::shares::Root;
use plain_rs::files::shares::Service;
use plain_rs::types::Instant;
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
fn record(row: ShareRow) -> async_graphql::Result<ShareRecord> {
    let roots = Service::roots(&row).map_err(|e| async_graphql::Error::new(e.to_string()))?;
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
    async fn share_record(
        &self,
        ctx: &Context<'_>,
        id: ID,
    ) -> async_graphql::Result<Option<ShareRecord>> {
        ctx.data::<Arc<Db>>()?
            .share_get(&id)?
            .map(record)
            .transpose()
    }
    async fn share_records(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<ShareRecord>> {
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
    ) -> async_graphql::Result<ShareRecord> {
        let service = ctx.data::<Arc<Service>>()?.clone();
        record(
            blocking(move || {
                service.create(
                    name,
                    real_paths,
                    read_only,
                    expires_at.map(|i| i.0.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
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
    ) -> async_graphql::Result<ShareRecord> {
        let service = ctx.data::<Arc<Service>>()?.clone();
        record(
            blocking(move || {
                service.update(
                    &id,
                    &name,
                    expires_at.map(|i| i.0.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
                    real_paths,
                )
            })
            .await?,
        )
    }
    async fn delete_share(&self, ctx: &Context<'_>, id: ID) -> async_graphql::Result<bool> {
        Ok(ctx.data::<Arc<Db>>()?.share_delete(&id)? > 0)
    }
}

async fn blocking<T: Send + 'static>(
    operation: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
) -> async_graphql::Result<T> {
    tokio::task::spawn_blocking(operation)
        .await?
        .map_err(|e| async_graphql::Error::new(e.to_string()))
}

#[cfg(test)]
#[path = "../../../../tests/unit/graphql/resolvers/shares/host.rs"]
mod tests;
