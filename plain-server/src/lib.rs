pub mod graphql;
pub mod http;
#[cfg(feature = "core")]
pub mod native;
#[cfg(feature = "core")]
pub mod server;

#[cfg(feature = "core")]
pub mod host;

#[cfg(all(test, feature = "core"))]
#[path = "../../tests/plain-server/support/mod.rs"]
pub(crate) mod test_support;

#[cfg(feature = "core")]
mod managed_peers;
