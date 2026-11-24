use super::{Call, FEATURE, call};
use crate::graphql::types::facts::{host_json, provider_params};
use crate::http::auth::public_gate;
use plain_graphql::{Context, Object};
use plain_rs::features::capability::Capability;
use plain_rs::features::prefs::Prefs;
use plain_rs::features::services::capabilities;
use plain_rs::infrastructure::host::bridge::Host;
use std::sync::Arc;

#[derive(Default)]
pub struct CallsQuery;

#[Object]
impl CallsQuery {
    async fn calls(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
    ) -> plain_graphql::Result<Vec<Call>> {
        public_gate::require(ctx, ctx.data_unchecked::<Arc<Prefs>>(), &[FEATURE])?;
        if public_gate::is_local(ctx) && !public_gate::require_granted(ctx, &[FEATURE]).await? {
            return Ok(Vec::new());
        }
        capabilities::require(ctx.data_unchecked::<Arc<Host>>(), Capability::Telephony)
            .await
            .map_err(|error| plain_graphql::Error::from(error))?;

        let facts = host_json(
            ctx,
            "systemCallFacts",
            provider_params(
                ctx,
                plain_rs::features::services::files::provider_query::Provider::Call,
                &query,
                plain_libs::serde_json::json!({"offset": offset, "limit": limit}),
            )
            .await?,
        )
        .await?;
        let host = ctx.data_unchecked::<Arc<Host>>();
        let locale = if !facts.is_empty() && ctx.look_ahead().field("geo").exists() {
            host.call("systemPhoneLocaleFacts", plain_libs::serde_json::json!({}))
                .await
                .ok()
                .and_then(|facts| {
                    plain_libs::serde_json::from_value::<plain_libs::phone::batch::LocaleFacts>(
                        facts,
                    )
                    .ok()
                })
        } else {
            None
        };
        let runtime = ctx.data_unchecked::<Arc<plain_libs::phone::Service>>();
        let mut calls: Vec<Call> = facts.iter().map(call).collect();
        if let Some(locale) = locale {
            let numbers = calls.iter().map(|call| call.number.clone()).collect();
            let metadata =
                plain_libs::phone::batch::lookup_many(runtime.clone(), numbers, locale).await?;
            for (call, geo) in calls.iter_mut().zip(metadata) {
                call.geo = geo.map(Into::into);
            }
        }
        Ok(calls)
    }

    async fn call_count(&self, ctx: &Context<'_>, query: String) -> plain_graphql::Result<i32> {
        if !public_gate::require_granted(ctx, &[FEATURE]).await? {
            return Ok(0);
        }
        capabilities::require(ctx.data_unchecked::<Arc<Host>>(), Capability::Telephony)
            .await
            .map_err(|error| plain_graphql::Error::from(error))?;
        let count = ctx
            .data_unchecked::<Arc<Host>>()
            .call(
                "systemCallCount",
                provider_params(
                    ctx,
                    plain_rs::features::services::files::provider_query::Provider::Call,
                    &query,
                    plain_libs::serde_json::json!({}),
                )
                .await?,
            )
            .await
            .map_err(|error| plain_graphql::Error::new(error))?;
        Ok(count.as_i64().unwrap_or_default() as i32)
    }
}
