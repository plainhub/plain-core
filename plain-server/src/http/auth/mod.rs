#[cfg(feature = "core")]
pub mod public_gate;

#[cfg(feature = "core")]
pub mod replay;

#[cfg(feature = "core")]
pub mod ws_login;

#[cfg(feature = "core")]
pub(crate) mod local_client;

#[cfg(feature = "core")]
pub(crate) mod cors;
