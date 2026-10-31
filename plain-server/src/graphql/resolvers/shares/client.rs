use super::client_inputs::{SharedEntryInput, SharedLinkInput, SharedMessageCardInput};
use super::client_models::{SharedBrowseResult, SharedLink};
use crate::server::ServerState;
use async_graphql::{Context, Object};
use plain_rs::chat::messages::share_card;
use plain_rs::services::peers::shared_client;

fn error(value: anyhow::Error) -> async_graphql::Error {
    async_graphql::Error::new(value.to_string())
}

#[derive(Default)]
pub struct SharedClientQuery;
#[Object]
impl SharedClientQuery {
    async fn share_link(
        &self,
        ctx: &Context<'_>,
        id: async_graphql::ID,
        host: Option<String>,
    ) -> async_graphql::Result<SharedLink> {
        Ok(
            shared_client::own_link(ctx.data::<ServerState>()?, &id, host)
                .await
                .map_err(error)?
                .into(),
        )
    }
    async fn initial_shared_link(
        &self,
        ctx: &Context<'_>,
        message_id: async_graphql::ID,
        expected: SharedMessageCardInput,
    ) -> async_graphql::Result<SharedLink> {
        let expected: share_card::Card = expected.try_into()?;
        Ok(
            shared_client::initial_link(ctx.data::<ServerState>()?, &message_id, &expected)
                .await
                .map_err(error)?
                .into(),
        )
    }
}
#[derive(Default)]
pub struct SharedClientMutation;
#[Object]
impl SharedClientMutation {
    async fn cache_shared_file(
        &self,
        ctx: &Context<'_>,
        link: SharedLinkInput,
        url_token: String,
        entry: SharedEntryInput,
        thumbnail: bool,
    ) -> async_graphql::Result<String> {
        shared_client::cache_file(
            ctx.data::<ServerState>()?,
            &ctx.data::<plain_rs::runtime::Runtime>()?.remote_cache,
            link.try_into()?,
            url_token,
            entry.into(),
            thumbnail,
        )
        .await
        .map_err(error)
    }
    async fn browse_shared_link(
        &self,
        ctx: &Context<'_>,
        message_id: async_graphql::ID,
        expected: SharedMessageCardInput,
        virtual_path: Option<String>,
    ) -> async_graphql::Result<Option<SharedBrowseResult>> {
        let expected: share_card::Card = expected.try_into()?;
        shared_client::browse(
            ctx.data::<ServerState>()?,
            &message_id,
            &expected,
            virtual_path.as_deref(),
        )
        .await
        .map_err(error)?
        .map(TryInto::try_into)
        .transpose()
    }
}
