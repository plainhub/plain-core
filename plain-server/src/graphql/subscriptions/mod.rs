mod app_state;
mod chat;
mod casting;
mod device;
pub(crate) mod events;
mod library;
mod mirror;
pub(crate) mod models;
mod notifications;
pub(crate) mod operation;
pub mod registry;
pub mod session;
mod sms;

#[derive(async_graphql::MergedSubscription, Default)]
pub struct Subscription(
    app_state::AppStateSubscriptions,
    casting::CastingSubscriptions,
    device::DeviceSubscriptions,
    mirror::MirrorSubscriptions,
    notifications::NotificationsSubscriptions,
    chat::ChatSubscriptions,
    library::LibrarySubscriptions,
    sms::SmsSubscriptions,
);
