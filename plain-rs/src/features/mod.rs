#[cfg(feature = "chat")]
pub mod chat;

#[cfg(feature = "library")]
pub mod clipboard;

#[cfg(feature = "chat")]
pub mod discovery;

#[cfg(feature = "library")]
pub mod enums;

#[cfg(feature = "library")]
pub mod favorites;

#[cfg(feature = "feeds")]
pub mod feeds;

#[cfg(any(feature = "chat", feature = "filesystem", feature = "library"))]
pub mod files;

#[cfg(feature = "library")]
pub mod media;

#[cfg(feature = "library")]
pub mod notes;

#[cfg(feature = "chat")]
pub mod peers;

#[cfg(feature = "prefs")]
pub mod prefs;

#[cfg(feature = "kv")]
pub mod recent_files;

#[cfg(feature = "runtime")]
pub mod services;

#[cfg(feature = "library")]
pub mod sessions;

#[cfg(feature = "kv")]
pub mod sms_sends;

#[cfg(feature = "library")]
pub mod tags;

#[cfg(feature = "prefs")]
pub mod updates;
