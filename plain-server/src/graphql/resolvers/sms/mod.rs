pub mod mutation;
pub mod query;

use crate::graphql::resolvers::tags::domain_types::Tag;
use crate::graphql::types::facts::{flag, instant, integer, list, text};
use plain_graphql::{Context, ID, SimpleObject};
use plain_libs::serde_json::Value;
use plain_rs::features::access::AccessFeature;
use plain_rs::features::services::phone_query::SmsType;
use plain_rs::features::services::sms::query as sms_query;
use plain_rs::infrastructure::db::Db;
use plain_rs::infrastructure::host::bridge::Host;
use std::sync::Arc;

#[derive(SimpleObject, Clone, Debug)]
pub struct Sim {
    pub id: plain_graphql::ID,
    pub label: String,
    pub number: String,
    #[graphql(name = "subscriptionId")]
    pub subscription_id: i32,
}

#[derive(SimpleObject)]
pub struct SmsSendReceipt {
    pub request_id: String,
    pub requires_confirmation: bool,
}

const SMS: AccessFeature = AccessFeature::Sms;

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
    pub sent_at: plain_rs::infrastructure::types::Instant,
    #[graphql(name = "serviceCenter")]
    pub service_center: String,
    pub read: bool,
    #[graphql(name = "threadId")]
    pub thread_id: ID,
    pub r#type: SmsType,
    #[graphql(name = "subscriptionId")]
    pub subscription_id: i32,
    #[graphql(name = "isMms")]
    pub is_mms: bool,
    pub attachments: Vec<SmsAttachment>,
}

#[plain_graphql::ComplexObject]
impl Sms {
    async fn tags(&self, ctx: &plain_graphql::Context<'_>) -> plain_graphql::Result<Vec<Tag>> {
        crate::graphql::resolvers::tags::loader::load(
            ctx,
            plain_rs::features::enums::DataType::Sms,
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
    pub last_message_at: plain_rs::infrastructure::types::Instant,
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

async fn count(ctx: &Context<'_>, request: sms_query::Request) -> plain_graphql::Result<i32> {
    let receipt = sms_query::execute(
        ctx.data_unchecked::<Arc<Db>>(),
        ctx.data_unchecked::<Arc<Host>>(),
        request,
    )
    .await
    .map_err(|error| plain_graphql::Error::from(error))?;
    Ok(integer(&receipt, "count") as i32)
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
        subscription_id: integer(item, "subscriptionId") as i32,
        is_mms: flag(item, "isMms"),
        attachments: list(item, "attachments", |attachment| SmsAttachment {
            path: text(attachment, "path"),
            content_type: text(attachment, "contentType"),
            name: text(attachment, "name"),
        }),
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/sms/mod.rs"]
mod tests;
