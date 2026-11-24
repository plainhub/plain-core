pub mod domain_types;
pub(crate) mod loader;

pub mod mutation;
pub mod query;

use plain_graphql::Context;
pub use plain_rs::features::enums::DataType;
use plain_rs::infrastructure::db::Db;
use std::sync::Arc;

fn failed(error: plain_rs::infrastructure::error::DomainError) -> plain_graphql::Error {
    plain_graphql::Error::from(error)
}

fn db<'a>(ctx: &'a Context<'_>) -> &'a Arc<Db> {
    ctx.data_unchecked::<Arc<Db>>()
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/tags/mod.rs"]
mod tests;
