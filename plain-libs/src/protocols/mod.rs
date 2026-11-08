#[cfg(feature = "http-client")]
pub mod http_client;

#[cfg(feature = "https")]
pub mod https;
pub mod mdns;

#[cfg(feature = "websocket")]
pub mod websocket;

#[cfg(feature = "multipart")]
pub mod multipart;
