use super::{SMS, SmsSendReceipt};
use crate::graphql::types::ActionResult;
use crate::http::auth::public_gate;
use plain_graphql::{Context, ID, Object};
use plain_rs::features::prefs::Prefs;
use plain_rs::features::services::sms::{query as sms_query, send as sms_send, state as sms_state};
use plain_rs::infrastructure::db::Db;
use plain_rs::infrastructure::host::bridge::Host;
use std::sync::Arc;

#[derive(Default)]
pub struct SmsMutation;

#[Object]
impl SmsMutation {
    async fn archive_sms_conversation(
        &self,
        ctx: &Context<'_>,
        id: ID,
        date: Option<plain_rs::infrastructure::types::Instant>,
    ) -> plain_graphql::Result<bool> {
        public_gate::require(ctx, ctx.data_unchecked::<Arc<Prefs>>(), &[SMS])?;
        let date = if let Some(date) = date {
            date.0
                .to_rfc3339_opts(plain_libs::chrono::SecondsFormat::Millis, true)
        } else {
            let receipt = sms_query::execute(
                ctx.data_unchecked::<Arc<Db>>(),
                ctx.data_unchecked::<Arc<Host>>(),
                sms_query::Request::ConversationDate { id: id.0.clone() },
            )
            .await
            .map_err(|error| plain_graphql::Error::from(error))?;
            receipt["date"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    plain_libs::chrono::Utc::now()
                        .to_rfc3339_opts(plain_libs::chrono::SecondsFormat::Millis, true)
                })
        };
        sms_state::archive(ctx.data_unchecked::<Arc<Db>>(), id.as_str(), &date)
            .map_err(|error| plain_graphql::Error::from(error))?;
        Ok(true)
    }

    fn unarchive_sms_conversation(&self, ctx: &Context<'_>, id: ID) -> plain_graphql::Result<bool> {
        public_gate::require(ctx, ctx.data_unchecked::<Arc<Prefs>>(), &[SMS])?;
        sms_state::unarchive(ctx.data_unchecked::<Arc<Db>>(), id.as_str())
            .map_err(|error| plain_graphql::Error::from(error))?;
        Ok(true)
    }

    async fn trash_sms(
        &self,
        ctx: &Context<'_>,
        query: String,
    ) -> plain_graphql::Result<ActionResult> {
        bulk(
            ctx,
            plain_rs::features::services::sms::mutations::Action::Trash,
            query,
        )
        .await
    }

    async fn restore_sms(
        &self,
        ctx: &Context<'_>,
        query: String,
    ) -> plain_graphql::Result<ActionResult> {
        bulk(
            ctx,
            plain_rs::features::services::sms::mutations::Action::Restore,
            query,
        )
        .await
    }

    async fn delete_sms(
        &self,
        ctx: &Context<'_>,
        query: String,
    ) -> plain_graphql::Result<ActionResult> {
        bulk(
            ctx,
            plain_rs::features::services::sms::mutations::Action::Delete,
            query,
        )
        .await
    }

    async fn send_sms(
        &self,
        ctx: &Context<'_>,
        number: String,
        body: String,
        #[graphql(default)] attachment_paths: Vec<String>,
        subscription_id: Option<i32>,
        request_id: String,
    ) -> plain_graphql::Result<SmsSendReceipt> {
        let receipt = sms_send::execute(
            ctx.data_unchecked::<Arc<Prefs>>(),
            ctx.data_unchecked::<Arc<Host>>(),
            ctx.data_unchecked::<Arc<plain_rs::features::services::sms::send_runtime::Runtime>>(),
            ctx.data_unchecked::<Arc<plain_rs::features::services::sms::mms::Runtime>>(),
            sms_send::Request {
                number,
                body,
                attachment_paths,
                subscription_id: subscription_id.filter(|id| *id >= 0),
                request_id,
            },
        )
        .await
        .map_err(|error| plain_graphql::Error::from(error))?;
        Ok(SmsSendReceipt {
            request_id: receipt.request_id,
            requires_confirmation: receipt.requires_confirmation,
        })
    }
}

async fn bulk(
    ctx: &Context<'_>,
    action: plain_rs::features::services::sms::mutations::Action,
    query: String,
) -> plain_graphql::Result<ActionResult> {
    public_gate::require(ctx, ctx.data_unchecked::<Arc<Prefs>>(), &[SMS])?;
    let affected_count = ctx
        .data_unchecked::<Arc<plain_rs::features::services::sms::mutations::Mutations>>()
        .execute(action, query)
        .await
        .map_err(|error| plain_graphql::Error::from(error))?;
    Ok(ActionResult { affected_count })
}
