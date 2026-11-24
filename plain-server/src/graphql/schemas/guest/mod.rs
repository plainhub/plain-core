mod query;

use crate::server::ServerState;
use plain_graphql::{EmptyMutation, EmptySubscription, Schema};
use query::Query;

struct Guest {
    state: ServerState,
    id: String,
}

fn schema_builder() -> plain_graphql::SchemaBuilder<Query, EmptyMutation, EmptySubscription> {
    Schema::build(Query, EmptyMutation, EmptySubscription)
}

pub(crate) fn schema(
    state: ServerState,
    id: String,
) -> Schema<Query, EmptyMutation, EmptySubscription> {
    schema_builder().data(Guest { state, id }).finish()
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/schemas/guest/mod.rs"]
mod tests;
