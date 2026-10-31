//! Public `/graphql` call-log roots.
//!
//! Reads reuse the same provider search the contact roots use, so a call and
//! a contact from the same query DSL behave alike.

use crate::graphql::resolvers::tags::domain_types::Tag;
use crate::graphql::types::ActionResult;
use crate::graphql::types::facts::instant;
use crate::graphql::types::facts::integer;
use crate::graphql::types::facts::text;
use crate::graphql::types::facts::{host_json, provider_params};
use crate::http::auth::public_gate;
use async_graphql::{Context, Enum, ID, Object, SimpleObject};
use plain_rs::db::Db;
use plain_rs::host::bridge::Host;
use plain_rs::prefs::Prefs;
use plain_rs::services::capabilities;
use plain_rs::services::files::provider_query::Provider;
use serde_json::Value;
use std::sync::Arc;

const READ: &str = "READ_CALL_LOG";
const WRITE: &str = "WRITE_CALL_LOG";

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, Default)]
pub enum CallType {
    Incoming = 1,
    Outgoing = 2,
    Missed = 3,
    Voicemail = 4,
    Rejected = 5,
    Blocked = 6,
    AnsweredExternally = 7,
    #[default]
    Unknown,
}

impl CallType {
    fn from_android(value: i64) -> Self {
        match value {
            1 => Self::Incoming,
            2 => Self::Outgoing,
            3 => Self::Missed,
            4 => Self::Voicemail,
            5 => Self::Rejected,
            6 => Self::Blocked,
            7 => Self::AnsweredExternally,
            _ => Self::Unknown,
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct PhoneGeo {
    pub country: String,
    #[graphql(name = "numberType")]
    pub number_type: String,
    pub carrier: String,
    pub description: String,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(complex)]
pub struct Call {
    pub type_code: i32,
    pub photo_uri: String,
    pub id: ID,
    pub number: String,
    pub name: String,
    #[graphql(name = "photoId")]
    pub photo_id: String,
    #[graphql(name = "startedAt")]
    pub started_at: plain_rs::types::Instant,
    #[graphql(name = "durationSec")]
    pub duration_sec: i32,
    pub r#type: CallType,
    #[graphql(name = "accountId")]
    pub account_id: ID,
    pub geo: Option<PhoneGeo>,
}

impl From<plain_rs::phone::Metadata> for PhoneGeo {
    fn from(value: plain_rs::phone::Metadata) -> Self {
        Self {
            country: value.country,
            number_type: value.number_type,
            carrier: value.carrier,
            description: value.description,
        }
    }
}

#[async_graphql::ComplexObject]
impl Call {
    async fn tags(&self, ctx: &async_graphql::Context<'_>) -> async_graphql::Result<Vec<Tag>> {
        crate::graphql::resolvers::tags::loader::load(
            ctx,
            plain_rs::enums::DataType::Call,
            self.id.as_str(),
        )
        .await
    }
}

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
    ) -> async_graphql::Result<Vec<Call>> {
        public_gate::require(ctx, ctx.data_unchecked::<Arc<Prefs>>(), &[READ])?;
        if public_gate::is_local(ctx) && !public_gate::require_granted(ctx, &[READ]).await? {
            return Ok(Vec::new());
        }
        capabilities::require(ctx.data_unchecked::<Arc<Host>>(), "CALLS")
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        let facts = host_json(
            ctx,
            "systemCallFacts",
            provider_params(
                ctx,
                plain_rs::services::files::provider_query::Provider::Call,
                &query,
                serde_json::json!({"offset": offset, "limit": limit}),
            )
            .await?,
        )
        .await?;
        let host = ctx.data_unchecked::<Arc<Host>>();
        let locale = if !facts.is_empty() && ctx.look_ahead().field("geo").exists() {
            host.call("systemPhoneLocaleFacts", serde_json::json!({}))
                .await
                .ok()
                .and_then(|facts| {
                    serde_json::from_value::<plain_rs::phone::batch::LocaleFacts>(facts).ok()
                })
        } else {
            None
        };
        let runtime = ctx.data_unchecked::<Arc<plain_rs::phone::Service>>();
        let prefs = ctx.data_unchecked::<Arc<Prefs>>();
        let mut calls = Vec::with_capacity(facts.len());
        for fact in &facts {
            let mut row = call(fact);
            let path = text(fact, "photoUri");
            row.photo_id = if path.is_empty()
                || path.to_ascii_lowercase().starts_with("http://")
                || path.to_ascii_lowercase().starts_with("https://")
            {
                path
            } else {
                plain_rs::chat::messages::content::make_file_id(
                    &path,
                    &prefs.get_or(plain_rs::prefs::pref_keys::URL_TOKEN, String::new()),
                )
            };
            calls.push(row);
        }
        if let Some(locale) = locale {
            let numbers = calls.iter().map(|call| call.number.clone()).collect();
            let metadata =
                plain_rs::phone::batch::lookup_many(runtime.clone(), numbers, locale).await?;
            for (call, geo) in calls.iter_mut().zip(metadata) {
                call.geo = geo.map(Into::into);
            }
        }
        Ok(calls)
    }

    async fn call_count(&self, ctx: &Context<'_>, query: String) -> async_graphql::Result<i32> {
        if !public_gate::require_granted(ctx, &[WRITE]).await? {
            return Ok(0);
        }
        capabilities::require(ctx.data_unchecked::<Arc<Host>>(), "CALLS")
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        let count = ctx
            .data_unchecked::<Arc<Host>>()
            .call(
                "systemCallCount",
                provider_params(
                    ctx,
                    plain_rs::services::files::provider_query::Provider::Call,
                    &query,
                    serde_json::json!({}),
                )
                .await?,
            )
            .await
            .map_err(|error| async_graphql::Error::new(error))?;
        Ok(count.as_i64().unwrap_or_default() as i32)
    }
}

#[derive(Default)]
pub struct CallsMutation;

#[Object]
impl CallsMutation {
    async fn delete_calls(
        &self,
        ctx: &Context<'_>,
        query: String,
    ) -> async_graphql::Result<ActionResult> {
        if query.trim().is_empty() {
            return Err(async_graphql::Error::new(
                "query is required for bulk mutations — pass 'all:true' to explicitly target everything",
            ));
        }
        public_gate::require(ctx, ctx.data_unchecked::<Arc<Prefs>>(), &[WRITE])?;
        // Call contributes no clause of its own, so a query carrying only
        // fields it does not act on resolves to the entire call log.
        plain_rs::phone::query::require_narrowing(
            ctx.data_unchecked::<Arc<Db>>(),
            Provider::Call,
            &query,
        )
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        capabilities::require(ctx.data_unchecked::<Arc<Host>>(), "CALLS")
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        let ids: Vec<Value> = host_json(
            ctx,
            "systemCallIds",
            provider_params(
                ctx,
                plain_rs::services::files::provider_query::Provider::Call,
                &query,
                serde_json::json!({}),
            )
            .await?,
        )
        .await?
        .iter()
        .filter_map(Value::as_str)
        .map(Value::from)
        .collect();
        let deleted: Vec<Value> = serde_json::from_value(
            ctx.data_unchecked::<Arc<Host>>()
                .call(
                    "systemDeleteRecords",
                    serde_json::json!({"provider": "CALL", "ids": ids}),
                )
                .await
                .map_err(|error| async_graphql::Error::new(error))?,
        )
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
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
    ) -> async_graphql::Result<bool> {
        public_gate::require(ctx, ctx.data_unchecked::<Arc<Prefs>>(), &["CALL_PHONE"])?;
        capabilities::require(ctx.data_unchecked::<Arc<Host>>(), "CALL_PHONE")
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        ctx.data_unchecked::<Arc<Host>>()
            .call(
                "systemMakeCall",
                serde_json::json!({"number": number, "showDialer": show_dialer}),
            )
            .await
            .map_err(|error| async_graphql::Error::new(error))?;
        Ok(true)
    }
}

fn call(fact: &Value) -> Call {
    let id = text(fact, "id");
    Call {
        id: ID::from(id),
        number: text(fact, "number"),
        name: text(fact, "name"),
        photo_id: String::new(),
        started_at: instant(fact, "startedAt"),
        duration_sec: integer(fact, "durationSec") as i32,
        r#type: CallType::from_android(integer(fact, "type")),
        type_code: integer(fact, "type") as i32,
        photo_uri: text(fact, "photoUri"),
        account_id: ID::from(text(fact, "accountId")),
        geo: None,
    }
}

#[cfg(test)]
#[path = "../../../../../tests/unit/graphql/resolvers/mobile/phone_calls/resolvers.rs"]
mod tests;
