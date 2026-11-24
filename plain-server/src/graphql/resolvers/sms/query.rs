use super::{SMS, Sim, Sms, SmsConversation, SmsCounts, count, message};
use crate::graphql::types::facts::{flag, instant, integer, strings, text};
use crate::http::auth::public_gate;
use plain_graphql::{Context, ID, Object};
use plain_libs::serde_json::Value;
use plain_rs::features::access::AccessFeature;
use plain_rs::features::prefs::Prefs;
use plain_rs::features::services::sms::query as sms_query;
use plain_rs::infrastructure::db::Db;
use plain_rs::infrastructure::host::bridge::Host;
use std::sync::Arc;

#[derive(Default)]
pub struct SmsQuery;

#[Object]
impl SmsQuery {
    async fn sims(&self, ctx: &Context<'_>) -> plain_graphql::Result<Vec<Sim>> {
        public_gate::require(
            ctx,
            ctx.data_unchecked::<Arc<Prefs>>(),
            &[AccessFeature::PhoneNumber],
        )?;
        let facts = ctx
            .data_unchecked::<Arc<Host>>()
            .call("systemSimFacts", plain_libs::serde_json::json!({}))
            .await
            .map_err(plain_graphql::Error::new)?;
        Ok(crate::graphql::types::facts::rows(&facts, |value| Sim {
            id: crate::graphql::types::facts::id(value, "id"),
            label: crate::graphql::types::facts::text(value, "label"),
            number: crate::graphql::types::facts::text(value, "number"),
            subscription_id: crate::graphql::types::facts::integer(value, "subscriptionId") as i32,
        }))
    }

    async fn sms(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
    ) -> plain_graphql::Result<Vec<Sms>> {
        if !public_gate::require_granted(ctx, &[SMS]).await? {
            return Ok(Vec::new());
        }
        let receipt = sms_query::execute(
            ctx.data_unchecked::<Arc<Db>>(),
            ctx.data_unchecked::<Arc<Host>>(),
            sms_query::Request::Search {
                query,
                offset,
                limit,
                include_trashed: false,
            },
        )
        .await
        .map_err(|error| plain_graphql::Error::from(error))?;
        Ok(receipt["items"]
            .as_array()
            .map(|items| items.iter().map(|item| message(item)).collect())
            .unwrap_or_default())
    }

    async fn sms_count(&self, ctx: &Context<'_>, query: String) -> plain_graphql::Result<i32> {
        if !public_gate::require_granted(ctx, &[SMS]).await? {
            return Ok(0);
        }
        count(ctx, sms_query::Request::Count { query }).await
    }

    async fn sms_box_counts(&self, ctx: &Context<'_>) -> plain_graphql::Result<SmsCounts> {
        if !public_gate::require_granted(ctx, &[SMS]).await? {
            return Ok(SmsCounts::default());
        }
        let receipt = sms_query::execute(
            ctx.data_unchecked::<Arc<Db>>(),
            ctx.data_unchecked::<Arc<Host>>(),
            sms_query::Request::Counts,
        )
        .await
        .map_err(|error| plain_graphql::Error::from(error))?;
        Ok(SmsCounts {
            total: integer(&receipt, "total") as i32,
            inbox: integer(&receipt, "inbox") as i32,
            sent: integer(&receipt, "sent") as i32,
            drafts: integer(&receipt, "drafts") as i32,
        })
    }

    async fn sms_conversations(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
    ) -> plain_graphql::Result<Vec<SmsConversation>> {
        if !public_gate::require_granted(ctx, &[SMS]).await? {
            return Ok(Vec::new());
        }
        conversations(ctx, query, offset, limit, false).await
    }

    async fn sms_conversation_count(
        &self,
        ctx: &Context<'_>,
        query: String,
    ) -> plain_graphql::Result<i32> {
        if !public_gate::require_granted(ctx, &[SMS]).await? {
            return Ok(0);
        }
        count(ctx, sms_query::Request::ConversationCount { query }).await
    }

    async fn archived_sms_conversations(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
    ) -> plain_graphql::Result<Vec<SmsConversation>> {
        if !public_gate::require_granted(ctx, &[SMS]).await? {
            return Ok(Vec::new());
        }
        conversations(ctx, query, offset, limit, true).await
    }
}

async fn conversations(
    ctx: &Context<'_>,
    query: String,
    offset: i32,
    limit: i32,
    archived: bool,
) -> plain_graphql::Result<Vec<SmsConversation>> {
    let request = if archived {
        sms_query::Request::ArchivedConversations {
            query,
            offset,
            limit,
        }
    } else {
        sms_query::Request::Conversations {
            query,
            offset,
            limit,
        }
    };
    let receipt = sms_query::execute(
        ctx.data_unchecked::<Arc<Db>>(),
        ctx.data_unchecked::<Arc<Host>>(),
        request,
    )
    .await
    .map_err(|error| plain_graphql::Error::from(error))?;
    Ok(receipt["items"]
        .as_array()
        .map(|items| items.iter().map(conversation).collect())
        .unwrap_or_default())
}

fn conversation(item: &Value) -> SmsConversation {
    SmsConversation {
        id: ID::from(text(item, "id")),
        address: text(item, "address"),
        snippet: text(item, "snippet"),
        last_message_at: instant(item, "date"),
        message_count: integer(item, "messageCount") as i32,
        read: flag(item, "read"),
        addresses: strings(item, "addresses"),
    }
}
