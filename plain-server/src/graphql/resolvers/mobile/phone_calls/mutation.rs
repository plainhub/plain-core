use super::FEATURE;
use crate::graphql::types::ActionResult;
use crate::graphql::types::facts::{host_json, provider_params};
use crate::http::auth::public_gate;
use plain_graphql::{Context, Object};
use plain_libs::serde_json::Value;
use plain_rs::features::access::AccessFeature;
use plain_rs::features::capability::Capability;
use plain_rs::features::prefs::Prefs;
use plain_rs::features::services::capabilities;
use plain_rs::features::services::files::provider_query::Provider;
use plain_rs::infrastructure::db::Db;
use plain_rs::infrastructure::host::bridge::Host;
use std::sync::Arc;

#[derive(Default)]
pub struct CallsMutation;

#[Object]
impl CallsMutation {
    async fn delete_calls(
        &self,
        ctx: &Context<'_>,
        query: String,
    ) -> plain_graphql::Result<ActionResult> {
        if query.trim().is_empty() {
            return Err(plain_graphql::Error::new(
                "query is required for bulk mutations — pass 'all:true' to explicitly target everything",
            ));
        }
        public_gate::require(ctx, ctx.data_unchecked::<Arc<Prefs>>(), &[FEATURE])?;
        // Call contributes no clause of its own, so a query carrying only
        // fields it does not act on resolves to the entire call log.
        plain_rs::features::services::phone_query::require_narrowing(
            ctx.data_unchecked::<Arc<Db>>(),
            Provider::Call,
            &query,
        )
        .map_err(|error| plain_graphql::Error::from(error))?;
        capabilities::require(ctx.data_unchecked::<Arc<Host>>(), Capability::Telephony)
            .await
            .map_err(|error| plain_graphql::Error::from(error))?;
        let ids: Vec<Value> = host_json(
            ctx,
            "systemCallIds",
            provider_params(
                ctx,
                plain_rs::features::services::files::provider_query::Provider::Call,
                &query,
                plain_libs::serde_json::json!({}),
            )
            .await?,
        )
        .await?
        .iter()
        .filter_map(Value::as_str)
        .map(Value::from)
        .collect();
        let deleted: Vec<Value> = plain_libs::serde_json::from_value(
            ctx.data_unchecked::<Arc<Host>>()
                .call(
                    "systemDeleteRecords",
                    plain_libs::serde_json::json!({"provider": "CALL", "ids": ids}),
                )
                .await
                .map_err(|error| plain_graphql::Error::new(error))?,
        )
        .map_err(|error| plain_graphql::Error::from(error))?;
        Ok(ActionResult {
            affected_count: deleted.len() as i32,
        })
    }

    /// `showDialer` hands the number to the dialer instead of placing the
    /// call outright.
    async fn call(
        &self,
        ctx: &Context<'_>,
        number: String,
        show_dialer: bool,
    ) -> plain_graphql::Result<bool> {
        public_gate::require(
            ctx,
            ctx.data_unchecked::<Arc<Prefs>>(),
            &[AccessFeature::CallPhone],
        )?;
        capabilities::require(ctx.data_unchecked::<Arc<Host>>(), Capability::Telephony)
            .await
            .map_err(|error| plain_graphql::Error::from(error))?;
        ctx.data_unchecked::<Arc<Host>>()
            .call(
                "systemMakeCall",
                plain_libs::serde_json::json!({"number": number, "showDialer": show_dialer}),
            )
            .await
            .map_err(|error| plain_graphql::Error::new(error))?;
        Ok(true)
    }
}
