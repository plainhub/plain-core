mod app_state;
mod casting;
mod chat;
mod device;
pub(crate) mod events;
mod files;
mod library;
mod mirror;
pub(crate) mod models;
mod notifications;
pub(crate) mod operation;

pub mod session;
pub(crate) mod sms;

#[derive(plain_graphql::MergedSubscription, Default)]
pub struct Subscription(
    app_state::AppStateSubscriptions,
    casting::CastingSubscriptions,
    device::DeviceSubscriptions,
    mirror::MirrorSubscriptions,
    notifications::NotificationsSubscriptions,
    chat::ChatSubscriptions,
    library::LibrarySubscriptions,
    files::FileSubscriptions,
    sms::SmsSubscriptions,
);
