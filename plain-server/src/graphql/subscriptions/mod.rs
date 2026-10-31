mod app_state;
mod casting;
mod chat;
mod device;
pub(crate) mod events;
mod library;
mod mirror;
pub(crate) mod models;
mod notifications;
pub(crate) mod operation;

pub mod session;
#[cfg(any(
    test,
    all(feature = "mobile", any(target_os = "android", target_os = "ios"))
))]
pub(crate) mod sms;

#[cfg(not(all(feature = "mobile", any(target_os = "android", target_os = "ios"))))]
use async_graphql::EmptySubscription as MobileSubscriptions;
#[cfg(all(feature = "mobile", any(target_os = "android", target_os = "ios")))]
use sms::SmsSubscriptions as MobileSubscriptions;

#[derive(async_graphql::MergedSubscription, Default)]
pub struct Subscription<Mobile: async_graphql::SubscriptionType + 'static = MobileSubscriptions>(
    app_state::AppStateSubscriptions,
    casting::CastingSubscriptions,
    device::DeviceSubscriptions,
    mirror::MirrorSubscriptions,
    notifications::NotificationsSubscriptions,
    chat::ChatSubscriptions,
    library::LibrarySubscriptions,
    Mobile,
);
