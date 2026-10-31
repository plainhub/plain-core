use async_graphql::Request;
use plain_rs::protocols::graphql::events::EVENTS;
use plain_rs::protocols::graphql::events::EventSpec;

pub(crate) fn spec(request: &Request) -> Result<&'static EventSpec, String> {
    use async_graphql::parser::{
        parse_query,
        types::{OperationType, Selection},
    };
    let document = parse_query(&request.query).map_err(|error| error.to_string())?;
    let operations = document
        .operations
        .iter()
        .filter(|(name, _)| {
            request
                .operation_name
                .as_deref()
                .is_none_or(|selected| name.is_some_and(|name| name.as_str() == selected))
        })
        .collect::<Vec<_>>();
    if operations.len() != 1 || operations[0].1.node.ty != OperationType::Subscription {
        return Err("Subscription document required".into());
    }
    let mut pending = operations[0]
        .1
        .node
        .selection_set
        .node
        .items
        .iter()
        .collect::<Vec<_>>();
    let mut visited = std::collections::HashSet::new();
    let mut names = std::collections::HashSet::new();
    while let Some(selection) = pending.pop() {
        match &selection.node {
            Selection::Field(field) => {
                names.insert((
                    field.node.response_key().node.as_str(),
                    field.node.name.node.as_str(),
                ));
            }
            Selection::InlineFragment(fragment) => {
                pending.extend(&fragment.node.selection_set.node.items)
            }
            Selection::FragmentSpread(spread) => {
                let name = &spread.node.fragment_name.node;
                if visited.insert(name.clone()) {
                    if let Some(fragment) = document.fragments.get(name) {
                        pending.extend(&fragment.node.selection_set.node.items);
                    }
                }
            }
        }
    }
    if names.len() != 1 {
        return Err("Subscription requires one root field".into());
    }
    EVENTS
        .iter()
        .filter(|spec| {
            !matches!(
                spec.event,
                "SMS_PROVIDER_CHANGED" | "SMS_SEND_RESULT" | "MMS_SEND_RESULT" | "MMS_SENT"
            ) || cfg!(all(
                feature = "mobile",
                any(target_os = "android", target_os = "ios")
            ))
        })
        .find(|spec| names.iter().any(|(_, name)| *name == spec.field))
        .ok_or_else(|| "Unknown subscription field".into())
}
