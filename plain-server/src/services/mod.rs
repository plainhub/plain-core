#[cfg(feature = "core")]
pub mod backups;
#[cfg(feature = "core")]
pub(crate) mod bookmarks;
#[cfg(feature = "core")]
pub(crate) mod cache;
#[cfg(feature = "core")]
pub mod feeds;
#[cfg(feature = "core")]
pub mod link_preview;
#[cfg(feature = "core")]
pub mod pomodoro;
#[cfg(feature = "core")]
pub(crate) mod pomodoro_sound;

pub mod discovery;

pub mod temp_values;

pub mod chat;

pub mod casting;

pub mod settings;

pub mod notifications;

pub mod files;

pub mod peers;

pub mod audio;

pub mod images;

#[cfg(feature = "core")]
pub(crate) mod sms;

#[cfg(feature = "core")]
pub(crate) mod sessions;

#[cfg(feature = "core")]
pub(crate) mod capabilities;

#[cfg(feature = "core")]
pub(crate) mod packages;

#[cfg(any(feature = "core", feature = "http_transport"))]
pub mod tls_identity;

#[cfg(feature = "core")]
pub(crate) mod device;

#[cfg(feature = "core")]
pub(crate) mod logs;

#[cfg(feature = "core")]
pub(crate) mod crash_reports;

#[cfg(feature = "core")]
pub(crate) mod http_server;

#[cfg(feature = "core")]
pub(crate) mod clipboard;
