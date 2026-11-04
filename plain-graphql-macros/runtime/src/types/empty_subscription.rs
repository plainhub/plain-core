use crate::{Context, Response, ServerError, SubscriptionType, registry};
use futures_util::stream::{self, Stream};
use std::{borrow::Cow, pin::Pin};
#[doc = " Empty subscription"]
#[doc = ""]
#[doc = " Only the parameters used to construct the Schema, representing an"]
#[doc = " unconfigured subscription."]
#[derive(Default, Copy, Clone)]
pub struct EmptySubscription;
impl SubscriptionType for EmptySubscription {
    fn type_name() -> Cow<'static, str> {
        Cow::Borrowed("EmptySubscription")
    }
    fn create_type_info(registry: &mut registry::Registry) -> String {
        registry.create_subscription_type::<Self, _>(|_| registry::MetaType::Object {
            name: "EmptySubscription".to_string(),
            description: None,
            fields: Default::default(),
            cache_control: Default::default(),

            is_subscription: true,
            rust_typename: Some(std::any::type_name::<Self>()),
        })
    }
    fn is_empty() -> bool {
        true
    }
    fn create_field_stream<'a>(
        &'a self,
        _ctx: &'a Context<'_>,
    ) -> Option<Pin<Box<dyn Stream<Item = Response> + Send + 'a>>>
    where
        Self: Send + Sync + 'static + Sized,
    {
        Some(Box::pin(stream::once(async move {
            let err = ServerError::new("Schema is not configured for subscription.", None);
            Response::from_errors(vec![err])
        })))
    }
}
