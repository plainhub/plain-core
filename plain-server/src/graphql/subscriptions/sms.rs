use super::events;
use super::models::{MmsSendResult, SmsProviderChange, SmsSendResult};
use futures_util::stream::BoxStream;
use plain_graphql::{Context, Result, Subscription};

#[derive(Default)]
pub struct SmsSubscriptions;
#[Subscription]
impl SmsSubscriptions {
    async fn sms_provider_changed(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<Option<SmsProviderChange>>>> {
        events::stream(ctx, "SMS_PROVIDER_CHANGED").await
    }
    async fn sms_send_result(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<SmsSendResult>>> {
        events::stream(ctx, "SMS_SEND_RESULT").await
    }
    async fn mms_send_result(
        &self,
        ctx: &Context<'_>,
    ) -> Result<BoxStream<'static, Result<MmsSendResult>>> {
        events::stream(ctx, "MMS_SEND_RESULT").await
    }
    async fn mms_sent(&self, ctx: &Context<'_>) -> Result<BoxStream<'static, Result<String>>> {
        events::stream(ctx, "MMS_SENT").await
    }
}
