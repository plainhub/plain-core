use crate::{graphql::types::parse_instant, server::ServerState};
use plain_graphql::{Context, ID, InputObject, Object, SimpleObject};
use plain_rs::{
    features::{
        files::shares::{self, Service, SharePath},
        services::peers::shared_client,
    },
    infrastructure::{db::ShareRow, types::Instant},
};

#[derive(SimpleObject)]
struct Share {
    id: ID,
    name: String,
    password: String,
    expires_at: Option<Instant>,
    read_only: bool,
    data: Vec<SharePath>,
    url: String,
}
#[derive(InputObject)]
struct SharePathInput {
    virtual_path: String,
    real_path: String,
    is_dir: bool,
}
#[derive(InputObject)]
struct ShareInput {
    name: String,
    password: String,
    expires_at: Option<Instant>,
    read_only: bool,
    data: Vec<SharePathInput>,
}
impl From<ShareInput> for shares::Input {
    fn from(input: ShareInput) -> Self {
        Self {
            name: input.name,
            password: input.password,
            expires_at: input.expires_at.map(|i| {
                i.0.to_rfc3339_opts(plain_libs::chrono::SecondsFormat::Millis, true)
            }),
            read_only: input.read_only,
            data: input
                .data
                .into_iter()
                .map(|path| SharePath {
                    virtual_path: path.virtual_path,
                    real_path: path.real_path,
                    is_dir: path.is_dir,
                })
                .collect(),
        }
    }
}
fn state<'a>(ctx: &'a Context<'_>) -> plain_graphql::Result<&'a ServerState> {
    let state = ctx.data::<ServerState>()?;
    crate::http::auth::public_gate::require(
        ctx,
        &state.prefs,
        &[plain_rs::features::access::AccessFeature::Files],
    )?;
    Ok(state)
}

async fn record(state: &ServerState, row: ShareRow) -> plain_graphql::Result<Share> {
    let data = Service::roots(&row)?;
    let url = shared_client::own_link(state, &row.id).await?.page_url;
    Ok(Share {
        id: row.id.into(),
        name: row.name,
        password: row.password,
        expires_at: row.expires_at.as_deref().map(parse_instant).transpose()?,
        read_only: row.read_only,
        data,
        url,
    })
}
#[derive(Default)]
pub struct ShareHostQuery;
#[Object]
impl ShareHostQuery {
    async fn share(&self, ctx: &Context<'_>, id: ID) -> plain_graphql::Result<Option<Share>> {
        let state = state(ctx)?;
        match state.db.share_get(&id)? {
            Some(row) => Ok(Some(record(state, row).await?)),
            None => Ok(None),
        }
    }
    async fn shares(&self, ctx: &Context<'_>) -> plain_graphql::Result<Vec<Share>> {
        let state = state(ctx)?;
        let mut shares = vec![];
        for row in state.db.share_list()? {
            shares.push(record(state, row).await?);
        }
        Ok(shares)
    }
}
#[derive(Default)]
pub struct ShareHostMutation;
#[Object]
impl ShareHostMutation {
    async fn create_share(
        &self,
        ctx: &Context<'_>,
        input: ShareInput,
    ) -> plain_graphql::Result<Share> {
        save(ctx, None, input).await
    }
    async fn update_share(
        &self,
        ctx: &Context<'_>,
        id: ID,
        input: ShareInput,
    ) -> plain_graphql::Result<Share> {
        save(ctx, Some(id), input).await
    }
    fn delete_share(&self, ctx: &Context<'_>, id: ID) -> plain_graphql::Result<bool> {
        Ok(state(ctx)?.db.share_delete(&id)? > 0)
    }
}
async fn save(
    ctx: &Context<'_>,
    id: Option<ID>,
    input: ShareInput,
) -> plain_graphql::Result<Share> {
    let state = state(ctx)?;
    let input: shares::Input = input.into();
    state
        .file_platform
        .authorize_metadata_paths(
            &input
                .data
                .iter()
                .map(|path| path.real_path.clone())
                .collect::<Vec<_>>(),
        )
        .await?;
    let service = Service::new(state.db.clone(), state.prefs.clone());
    let row = tokio::task::spawn_blocking(move || match id {
        Some(id) => service.update(&id, input),
        None => service.create(input),
    })
    .await??;
    record(state, row).await
}
#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/shares/host.rs"]
mod tests;
