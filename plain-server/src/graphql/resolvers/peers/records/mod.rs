pub mod query;

#[cfg(all(test, feature = "http_transport"))]
#[path = "../../../../../../tests/plain-server/unit/graphql/resolvers/peers/records/mod.rs"]
mod tests;
