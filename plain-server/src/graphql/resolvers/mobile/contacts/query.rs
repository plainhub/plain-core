use super::{FEATURE, contact, contact_rows, group};
use crate::graphql::resolvers::mobile::contacts::types::*;
use crate::graphql::types::facts::{host_json, provider_params, text};
use crate::http::auth::public_gate;
use plain_graphql::{Context, Object};
use plain_libs::serde_json::json;
use plain_rs::features::capability::Capability;
use plain_rs::features::prefs::Prefs;
use plain_rs::features::services::capabilities;
use plain_rs::infrastructure::host::bridge::Host;
use std::sync::Arc;

#[derive(Default)]
pub struct ContactsQuery;

#[Object]
impl ContactsQuery {
    async fn contacts(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
    ) -> plain_graphql::Result<Vec<Contact>> {
        public_gate::require(ctx, ctx.data_unchecked::<Arc<Prefs>>(), &[FEATURE])?;
        capabilities::require(ctx.data_unchecked::<Arc<Host>>(), Capability::Telephony)
            .await
            .map_err(|error| plain_graphql::Error::from(error))?;
        if public_gate::is_local(ctx) && !public_gate::require_granted(ctx, &[FEATURE]).await? {
            return Ok(Vec::new());
        }

        let facts = contact_rows(
            ctx,
            provider_params(
                ctx,
                plain_rs::features::services::files::provider_query::Provider::Contact,
                &query,
                plain_libs::serde_json::json!({"offset": offset, "limit": limit}),
            )
            .await?,
        )
        .await?;
        Ok(facts.iter().map(|fact| contact(fact)).collect())
    }

    async fn contact_count(&self, ctx: &Context<'_>, query: String) -> plain_graphql::Result<i32> {
        capabilities::require(ctx.data_unchecked::<Arc<Host>>(), Capability::Telephony)
            .await
            .map_err(|error| plain_graphql::Error::from(error))?;
        if !public_gate::require_granted(
            ctx,
            &[if public_gate::is_local(ctx) {
                FEATURE
            } else {
                FEATURE
            }],
        )
        .await?
        {
            return Ok(0);
        }
        let count = ctx
            .data_unchecked::<Arc<Host>>()
            .call(
                "systemContactCount",
                provider_params(
                    ctx,
                    plain_rs::features::services::files::provider_query::Provider::Contact,
                    &query,
                    plain_libs::serde_json::json!({}),
                )
                .await?,
            )
            .await
            .map_err(|error| plain_graphql::Error::new(error))?;
        Ok(count.as_i64().unwrap_or_default() as i32)
    }

    async fn contact_sources(
        &self,
        ctx: &Context<'_>,
    ) -> plain_graphql::Result<Vec<ContactSource>> {
        public_gate::require(ctx, ctx.data_unchecked::<Arc<Prefs>>(), &[FEATURE])?;
        capabilities::require(ctx.data_unchecked::<Arc<Host>>(), Capability::Telephony)
            .await
            .map_err(|error| plain_graphql::Error::from(error))?;
        Ok(plain_rs::features::services::contacts::projection::sources(
            host_json(ctx, "systemContactSources", json!({})).await?,
        )
        .iter()
        .map(|fact| ContactSource {
            name: text(fact, "name"),
            r#type: text(fact, "type"),
        })
        .collect())
    }

    async fn contact_groups(&self, ctx: &Context<'_>) -> plain_graphql::Result<Vec<ContactGroup>> {
        public_gate::require(ctx, ctx.data_unchecked::<Arc<Prefs>>(), &[FEATURE])?;
        capabilities::require(ctx.data_unchecked::<Arc<Host>>(), Capability::Telephony)
            .await
            .map_err(|error| plain_graphql::Error::from(error))?;
        Ok(plain_rs::features::services::contacts::projection::groups(
            host_json(ctx, "systemContactGroupFacts", json!({})).await?,
        )
        .iter()
        .map(group)
        .collect())
    }
}
