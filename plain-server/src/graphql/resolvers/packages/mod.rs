pub mod mutation;
pub mod types;

pub mod query;

use plain_rs::features::access::AccessFeature;

const FEATURE: AccessFeature = AccessFeature::Apps;

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/resolvers/packages/mod.rs"]
mod tests;
