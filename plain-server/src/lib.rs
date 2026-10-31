// `MergedObject` derives nest every mounted root type into one auto-trait
// proof, and the contract schema now mounts ~40 of them; the default
// limit overflows long before the roots themselves do.
#![recursion_limit = "512"]

pub mod graphql;
pub mod http;
#[cfg(feature = "core")]
pub mod native;
#[cfg(feature = "core")]
pub mod server;

#[cfg(feature = "core")]
pub mod host;

#[cfg(all(test, feature = "core"))]
#[path = "../tests/support/mod.rs"]
pub(crate) mod test_support;
