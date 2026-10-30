#[cfg(feature = "core")]
pub mod peer_address;

#[cfg(feature = "core")]
pub mod peer_download;


#[cfg(feature = "core")]
pub mod peer_lan;

#[cfg(feature = "core")]
pub mod peer_sdk;

#[cfg(feature = "core")]
pub mod peer_status_controller;

#[cfg(feature = "core")]
pub mod peer_transport;

#[cfg(feature = "core")]
pub mod peer_wire;

#[cfg(feature = "core")]
pub mod shared_batch;

#[cfg(feature = "core")]
pub mod shared_client;

#[cfg(feature = "core")]
pub mod shared_download;

#[cfg(feature = "core")]
pub mod shared_zip;

#[cfg(feature = "core")]
pub mod status_outgoing;

#[cfg(feature = "core")]
pub mod status_socket;

#[cfg(feature = "core")]
pub(crate) mod shared_transfer;

#[cfg(feature = "core")]
pub(crate) mod aware;

#[cfg(feature = "core")]
pub(crate) mod management;

#[cfg(feature = "core")]
mod management_wire;

#[cfg(feature = "core")]
pub(crate) mod management_http;

#[cfg(feature = "core")]
mod notifications;

#[cfg(feature = "core")]
mod metadata;

#[cfg(feature = "core")]
mod clipboard;

#[cfg(feature = "core")]
pub mod subscriptions;
