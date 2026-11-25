mod mutation;

mod query;

use crate::server::ServerState;
use mutation::Mutation;
use plain_graphql::{EmptySubscription, Schema};
use plain_rs::infrastructure::transport::auth::AuthenticatedPeer;
use query::Query;

struct PeerContext {
    state: ServerState,
    authenticated: AuthenticatedPeer,
    channel_id: String,
}

pub(crate) type PeerSchema = Schema<Query, Mutation, EmptySubscription>;

pub(crate) fn schema() -> PeerSchema {
    Schema::build(Query, Mutation, EmptySubscription)
        .register_output_type::<plain_rs::infrastructure::types::Long>()
        .register_output_type::<plain_rs::features::chat::enums::PeerStatus>()
        .limit_depth(16)
        .limit_complexity(256)
        .finish()
}

pub(crate) async fn execute(
    state: &ServerState,
    client_id: &str,
    channel_id: &str,
    body: &[u8],
) -> (u16, Vec<u8>) {
    use plain_rs::infrastructure::transport::auth;
    let result = auth::authenticate(&state.db, &state.prefs, client_id, channel_id, body);
    let authenticated = match result {
        Ok(authenticated) => authenticated,
        Err(error) => return (error.http_status(), vec![]),
    };
    let mut request = match plain_libs::serde_json::from_str::<plain_graphql::Request>(
        &authenticated.graphql_json,
    ) {
        Ok(request) => request,
        Err(_) => return (400, vec![]),
    };
    if authenticated.message_id.is_some() && !is_message_delivery(&mut request) {
        return (400, vec![]);
    }
    let key = authenticated.key.clone();
    let response = state
        .peer_schema
        .execute(request.data(PeerContext {
            state: state.clone(),
            authenticated,
            channel_id: channel_id.into(),
        }))
        .await;
    let response = crate::graphql::executor::error_codes(response);
    let context = plain_rs::infrastructure::transport::http_response::Context::new(body);
    match plain_libs::serde_json::to_vec(&response) {
        Ok(body) => context
            .encrypt(&key, &body)
            .map(|body| (200, body))
            .unwrap_or((500, vec![])),
        Err(_) => (500, vec![]),
    }
}

fn is_message_delivery(request: &mut plain_graphql::Request) -> bool {
    use plain_graphql::parser::types::OperationType;
    use plain_graphql::parser::types::Selection;
    let Ok(document) = request.parsed_query() else { return false };
    let mut operations = document.operations.iter();
    let Some((_, operation)) = operations.next() else { return false };
    if operations.next().is_some() || operation.node.ty != OperationType::Mutation {
        return false;
    }
    let mut selections = &operation.node.selection_set.node.items;
    let mut visited = std::collections::HashSet::new();
    while let [selection] = selections.as_slice() {
        match &selection.node {
            Selection::Field(field) => return field.node.name.node.as_str() == "createChatItem",
            Selection::InlineFragment(fragment) => selections = &fragment.node.selection_set.node.items,
            Selection::FragmentSpread(spread) => {
                let name = &spread.node.fragment_name.node;
                let Some(fragment) = document.fragments.get(name) else { return false };
                if !visited.insert(name) { return false; }
                selections = &fragment.node.selection_set.node.items;
            }
        }
    }
    false
}

#[cfg(test)]
#[path = "../../../../../tests/plain-server/unit/graphql/schemas/peer/mod.rs"]
mod tests;
