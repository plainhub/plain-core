#[cfg(feature = "core")]
pub mod types;

#[cfg(feature = "core")]
pub mod schemas;

#[cfg(feature = "core")]
pub mod resolvers;

#[cfg(feature = "core")]
pub mod executor;

#[cfg(feature = "core")]
pub(crate) mod enum_literals;

#[cfg(feature = "core")]
pub mod subscriptions;

#[cfg(feature = "core")]
pub mod enums;

#[cfg(all(test, feature = "core"))]
#[path = "../../../tests/plain-server/unit/graphql/mod.rs"]
mod macro_generation_tests;
