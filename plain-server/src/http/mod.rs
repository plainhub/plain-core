#[cfg(feature = "http_transport")]
pub mod listeners;

#[cfg(any(feature = "core", feature = "http_transport"))]
pub(crate) mod router;

#[cfg(any(feature = "core", feature = "http_transport"))]
pub(crate) mod health;

#[cfg(feature = "core")]
pub(crate) mod fs;

#[cfg(feature = "core")]
pub(crate) mod zip;

#[cfg(feature = "core")]
pub(crate) mod upload;

#[cfg(feature = "core")]
pub mod proxy;

#[cfg(feature = "core")]
pub(crate) mod graphql;

#[cfg(feature = "core")]
pub(crate) mod init;

#[cfg(feature = "core")]
pub(crate) mod nearby;

#[cfg(feature = "core")]
pub(crate) mod static_files;

#[cfg(feature = "core")]
pub(crate) mod file_response;

#[cfg(feature = "core")]
pub mod web_bundle;

#[cfg(feature = "core")]
pub(crate) mod dlna;

#[cfg(feature = "core")]
pub(crate) mod ble;

pub mod websocket;
pub mod auth;

#[derive(Clone)]
pub(crate) struct ConnectionScheme(pub(crate) &'static str);
