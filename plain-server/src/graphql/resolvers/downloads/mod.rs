mod inputs;
mod models;

pub mod mutation;
pub mod query;

fn error(error: plain_libs::anyhow::Error) -> plain_graphql::Error {
    plain_graphql::Error::from(error)
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/downloads/mod.rs"]
mod tests;
