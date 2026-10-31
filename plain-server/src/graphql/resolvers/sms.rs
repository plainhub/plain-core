use crate::graphql::resolvers::tags::domain_types::Tag;
use crate::graphql::types::ActionResult;
use crate::graphql::types::facts::flag;
use crate::graphql::types::facts::instant;
use crate::graphql::types::facts::integer;
use crate::graphql::types::facts::list;
use crate::graphql::types::facts::strings;
use crate::graphql::types::facts::text;
use crate::http::auth::public_gate;
use async_graphql::{Context, Enum, ID, Object, SimpleObject};
use plain_rs::db::Db;
use plain_rs::host::bridge::Host;
use plain_rs::prefs::Prefs;
use plain_rs::services::sms::query as sms_query;
use plain_rs::services::sms::send as sms_send;
use plain_rs::services::sms::state as sms_state;
use serde_json::Value;
use std::sync::Arc;

const READ: &str = "READ_SMS";

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug, Default)]
pub enum SmsType {
    Inbox = 1,
    Sent = 2,
    Draft = 3,
    Outbox = 4,
    Failed = 5,
    Queued = 6,
    #[default]
    Unknown,
}

impl SmsType {
    fn from_android(value: i64) -> Self {
        match value {
            1 => Self::Inbox,
            2 => Self::Sent,
            3 => Self::Draft,
            4 => Self::Outbox,
            5 => Self::Failed,
            6 => Self::Queued,
            _ => Self::Unknown,
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct SmsAttachment {
    pub path: String,
    #[graphql(name = "contentType")]
    pub content_type: String,
    pub name: String,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(complex)]
pub struct Sms {
    pub id: ID,
    pub body: String,
    pub address: String,
    #[graphql(name = "sentAt")]
    pub sent_at: plain_rs::types::Instant,
    #[graphql(name = "serviceCenter")]
    pub service_center: String,
    pub read: bool,
    #[graphql(name = "threadId")]
    pub thread_id: ID,
    pub r#type: SmsType,
    pub type_code: i32,
    #[graphql(name = "subscriptionId")]
    pub subscription_id: i32,
    #[graphql(name = "isMms")]
    pub is_mms: bool,
    pub attachments: Vec<SmsAttachment>,
}

#[async_graphql::ComplexObject]
impl Sms {
    async fn tags(&self, ctx: &async_graphql::Context<'_>) -> async_graphql::Result<Vec<Tag>> {
        crate::graphql::resolvers::tags::loader::load(
            ctx,
            plain_rs::enums::DataType::Sms,
            self.id.as_str(),
        )
        .await
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct SmsConversation {
    pub id: ID,
    pub address: String,
    pub snippet: String,
    #[graphql(name = "lastMessageAt")]
    pub last_message_at: plain_rs::types::Instant,
    #[graphql(name = "messageCount")]
    pub message_count: i32,
    pub read: bool,
    pub addresses: Vec<String>,
}

#[derive(SimpleObject, Clone, Debug, Default)]
pub struct SmsCounts {
    pub total: i32,
    pub inbox: i32,
    pub sent: i32,
    pub drafts: i32,
}

#[derive(Default)]
pub struct SmsQuery;

#[Object]
impl SmsQuery {
    async fn sms(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
    ) -> async_graphql::Result<Vec<Sms>> {
        if !public_gate::require_granted(ctx, &[READ]).await? {
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
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        Ok(receipt["items"]
            .as_array()
            .map(|items| items.iter().map(|item| message(item)).collect())
            .unwrap_or_default())
    }

    async fn sms_count(&self, ctx: &Context<'_>, query: String) -> async_graphql::Result<i32> {
        if !public_gate::require_granted(ctx, &[READ]).await? {
            return Ok(0);
        }
        count(ctx, sms_query::Request::Count { query }).await
    }

    async fn sms_box_counts(&self, ctx: &Context<'_>) -> async_graphql::Result<SmsCounts> {
        if !public_gate::require_granted(ctx, &[READ]).await? {
            return Ok(SmsCounts::default());
        }
        let receipt = sms_query::execute(
            ctx.data_unchecked::<Arc<Db>>(),
            ctx.data_unchecked::<Arc<Host>>(),
            sms_query::Request::Counts,
        )
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
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
    ) -> async_graphql::Result<Vec<SmsConversation>> {
        if !public_gate::require_granted(ctx, &[READ]).await? {
            return Ok(Vec::new());
        }
        conversations(ctx, query, offset, limit, false).await
    }

    async fn sms_conversation_count(
        &self,
        ctx: &Context<'_>,
        query: String,
    ) -> async_graphql::Result<i32> {
        if !public_gate::require_granted(ctx, &[READ]).await? {
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
    ) -> async_graphql::Result<Vec<SmsConversation>> {
        if !public_gate::require_granted(ctx, &[READ]).await? {
            return Ok(Vec::new());
        }
        conversations(ctx, query, offset, limit, true).await
    }
}

#[derive(Default)]
pub struct SmsMutation;

#[Object]
impl SmsMutation {
    async fn archive_sms_conversation(
        &self,
        ctx: &Context<'_>,
        id: ID,
        date: Option<plain_rs::types::Instant>,
    ) -> async_graphql::Result<bool> {
        let date = if let Some(date) = date {
            date.0.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        } else {
            let receipt = sms_query::execute(
                ctx.data_unchecked::<Arc<Db>>(),
                ctx.data_unchecked::<Arc<Host>>(),
                sms_query::Request::ConversationDate { id: id.0.clone() },
            )
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
            receipt["date"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                })
        };
        sms_state::archive(ctx.data_unchecked::<Arc<Db>>(), id.as_str(), &date)
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        Ok(true)
    }

    async fn unarchive_sms_conversation(
        &self,
        ctx: &Context<'_>,
        id: ID,
    ) -> async_graphql::Result<bool> {
        sms_state::unarchive(ctx.data_unchecked::<Arc<Db>>(), id.as_str())
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        Ok(true)
    }

    async fn trash_sms(
        &self,
        ctx: &Context<'_>,
        query: String,
    ) -> async_graphql::Result<ActionResult> {
        bulk(
            ctx,
            plain_rs::services::sms::mutations::Action::Trash,
            query,
        )
        .await
    }

    async fn restore_sms(
        &self,
        ctx: &Context<'_>,
        query: String,
    ) -> async_graphql::Result<ActionResult> {
        bulk(
            ctx,
            plain_rs::services::sms::mutations::Action::Restore,
            query,
        )
        .await
    }

    async fn delete_sms(
        &self,
        ctx: &Context<'_>,
        query: String,
    ) -> async_graphql::Result<ActionResult> {
        bulk(
            ctx,
            plain_rs::services::sms::mutations::Action::Delete,
            query,
        )
        .await
    }

    /// `subscriptionId` picks the SIM (-1 = system default, see `sims`);
    /// `requestId` is an optional idempotency key that dedupes retried sends.
    async fn send_sms(
        &self,
        ctx: &Context<'_>,
        number: String,
        body: String,
        subscription_id: i32,
        request_id: Option<String>,
    ) -> async_graphql::Result<bool> {
        sms_send::execute(
            ctx.data_unchecked::<Arc<Prefs>>(),
            ctx.data_unchecked::<Arc<Host>>(),
            ctx.data_unchecked::<Arc<plain_rs::services::sms::send_runtime::Runtime>>(),
            sms_send::Request {
                number,
                body,
                subscription_id: (subscription_id >= 0).then_some(subscription_id),
                client_id: None,
                client_request_id: request_id,
            },
        )
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        Ok(true)
    }

    /// Compose an MMS in the device's default SMS app and track the send.
    /// Returns a pendingId consumed by the MMS polling flow.
    async fn send_mms(
        &self,
        ctx: &Context<'_>,
        number: String,
        body: String,
        attachment_paths: Vec<String>,
        thread_id: ID,
    ) -> async_graphql::Result<String> {
        ctx.data_unchecked::<Arc<plain_rs::services::sms::mms::Runtime>>()
            .send(number, body, attachment_paths, thread_id.0)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))
    }
}

async fn count(ctx: &Context<'_>, request: sms_query::Request) -> async_graphql::Result<i32> {
    let receipt = sms_query::execute(
        ctx.data_unchecked::<Arc<Db>>(),
        ctx.data_unchecked::<Arc<Host>>(),
        request,
    )
    .await
    .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    Ok(integer(&receipt, "count") as i32)
}

async fn conversations(
    ctx: &Context<'_>,
    query: String,
    offset: i32,
    limit: i32,
    archived: bool,
) -> async_graphql::Result<Vec<SmsConversation>> {
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
    .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    Ok(receipt["items"]
        .as_array()
        .map(|items| items.iter().map(conversation).collect())
        .unwrap_or_default())
}

async fn bulk(
    ctx: &Context<'_>,
    action: plain_rs::services::sms::mutations::Action,
    query: String,
) -> async_graphql::Result<ActionResult> {
    let affected_count = ctx
        .data_unchecked::<Arc<plain_rs::services::sms::mutations::Mutations>>()
        .execute(action, query)
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
    Ok(ActionResult { affected_count })
}

fn message(item: &Value) -> Sms {
    let id = text(item, "id");
    Sms {
        id: ID::from(id),
        body: text(item, "body"),
        address: text(item, "address"),
        sent_at: instant(item, "date"),
        service_center: text(item, "serviceCenter"),
        read: flag(item, "read"),
        thread_id: ID::from(text(item, "threadId")),
        r#type: SmsType::from_android(integer(item, "type")),
        type_code: integer(item, "type") as i32,
        subscription_id: integer(item, "subscriptionId") as i32,
        is_mms: flag(item, "isMms"),
        attachments: list(item, "attachments", |attachment| SmsAttachment {
            path: text(attachment, "path"),
            content_type: text(attachment, "contentType"),
            name: text(attachment, "name"),
        }),
    }
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

#[cfg(test)]
#[path = "../../../tests/unit/graphql/resolvers/sms.rs"]
mod tests;
