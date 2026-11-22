use plain_rs::features::peers::error::PeerConnectionErrorCode;
pub(crate) mod resource;
pub(crate) mod upload;

use plain_libs::protocols::http_client;

pub(crate) fn request_error(error: plain_libs::anyhow::Error) -> String {
    if error
        .downcast_ref::<http_client::Error>()
        .is_some_and(http_client::Error::is_timeout)
    {
        return PeerConnectionErrorCode::ConnectionTimeout.to_string();
    }
    error.to_string()
}

pub(crate) fn safe_read(request: &plain_graphql::Request) -> bool {
    use plain_graphql::parser::{
        parse_query,
        types::{OperationType, Selection},
    };
    let Ok(document) = parse_query(&request.query) else {
        return false;
    };
    let mut selected = document.operations.iter().filter(|(name, _)| {
        request
            .operation_name
            .as_deref()
            .is_none_or(|op| name.is_some_and(|name| name.as_str() == op))
    });
    let Some((_, operation)) = selected.next() else {
        return false;
    };
    operation.node.ty == OperationType::Query
        && selected.next().is_none()
        && !operation.node.selection_set.node.items.is_empty()
        && operation.node.selection_set.node.items.iter().all(|selection| {
            matches!(
                &selection.node, Selection::Field(field) if matches!(field.node.name.node.as_str(),
                    "app" | "mounts" | "files" | "fileInfo" | "fileCount" | "appFiles" | "appFileCount" |
                    "indexedFiles" | "indexedFileCount" | "recentFiles" | "trashedFiles" | "trashedFileCount")
            )
        })
}

#[cfg(test)]
#[path = "../../tests/plain-server/unit/managed_peers.rs"]
mod tests;
