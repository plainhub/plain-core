#[cfg(feature = "core")]
use crate::server::ServerState;
use async_graphql::{ObjectType, Request, Response, Schema, SubscriptionType};
#[cfg(feature = "core")]
use plain_rs::events::WsEvent;

/// Where a GraphQL request came from, injected by the adapter that received
/// it — never by the document or its variables.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RequestOrigin {
    /// This device, over the native bridge. Holds real OS grants and file
    /// authorization; must not be judged by the web API permission list.
    Local,
    /// Another device, over HTTP. Answerable only where the user handed the
    /// web client the matching `api_permissions` entry.
    Remote,
}

#[derive(Clone)]
pub struct RequestClient {
    client_id: String,
}

impl RequestClient {
    pub fn client_id(&self) -> &str {
        &self.client_id
    }
}

#[derive(Clone)]
pub struct RequestContext {
    #[cfg(feature = "core")]
    state: Option<ServerState>,
    remote_client_id: Option<String>,
    notify_changes: bool,
    origin: RequestOrigin,
}
impl RequestContext {
    #[cfg(feature = "core")]
    pub(crate) fn local(state: ServerState) -> Self {
        Self {
            state: Some(state),
            remote_client_id: None,
            notify_changes: true,
            origin: RequestOrigin::Local,
        }
    }
    #[cfg(feature = "core")]
    pub(crate) fn remote(state: Option<ServerState>) -> Self {
        Self {
            state,
            remote_client_id: None,
            notify_changes: false,
            origin: RequestOrigin::Remote,
        }
    }
    /// The core's own shell, identified by its own client id. Same trust as
    /// the native bridge: real OS grants, no web permission lookup.
    #[cfg(feature = "core")]
    pub(crate) fn authenticated_local(state: ServerState, client_id: String) -> Self {
        let mut context = Self::remote(Some(state));
        context.remote_client_id = Some(client_id);
        context.origin = RequestOrigin::Local;
        context
    }
    #[cfg(feature = "core")]
    pub(crate) fn authenticated_remote(state: ServerState, client_id: String) -> Self {
        let mut context = Self::remote(Some(state));
        context.remote_client_id = Some(client_id);
        context
    }
    pub(crate) fn origin(&self) -> RequestOrigin {
        self.origin
    }
}
impl RequestContext {
    fn prepare(&self, request: Request) -> Result<Request, Response> {
        let origin = self.origin();
        let request = request.data(origin);
        #[cfg(feature = "core")]
        let request = match self.state.as_ref() {
            Some(state) => {
                let client_id = if origin == RequestOrigin::Local {
                    match state.native_client_id() {
                        Ok(client_id) => Some(client_id),
                        Err(error) => {
                            return Err(Response::from_errors(vec![
                                async_graphql::ServerError::new(error, None),
                            ]));
                        }
                    }
                } else {
                    self.remote_client_id.clone()
                };
                let request = request.data(state.clone());
                match client_id {
                    Some(client_id) => request
                        .data(RequestClient {
                            client_id: client_id.clone(),
                        })
                        .data(client_id),
                    None => request,
                }
            }
            None => request,
        };
        Ok(request)
    }
}

pub async fn execute_graphql<
    Q: ObjectType + 'static,
    M: ObjectType + 'static,
    S: SubscriptionType + 'static,
>(
    schema: &Schema<Q, M, S>,
    request: Request,
    context: RequestContext,
) -> Response {
    let changed = context.notify_changes && content_changes(&request);
    let request = match context.prepare(request) {
        Ok(request) => request,
        Err(response) => return response,
    };
    let response = schema.execute(request).await;
    if changed && response.errors.is_empty() {
        #[cfg(feature = "core")]
        if let Some(state) = context.state {
            let _ = state
                .events
                .send(WsEvent::broadcast("CONTENT_CHANGED", "{}".into()));
        }
    }
    response
}

pub(crate) fn content_changes(request: &Request) -> bool {
    use async_graphql::parser::{
        parse_query,
        types::{OperationType, Selection},
    };
    let Ok(document) = parse_query(&request.query) else {
        return false;
    };
    document
        .operations
        .iter()
        .filter(|(name, _)| {
            request
                .operation_name
                .as_deref()
                .is_none_or(|selected| name.is_some_and(|n| n.as_str() == selected))
        })
        .any(|(_, operation)| {
            if operation.node.ty != OperationType::Mutation {
                return false;
            }
            let mut pending = operation
                .node
                .selection_set
                .node
                .items
                .iter()
                .collect::<Vec<_>>();
            let mut visited = std::collections::HashSet::new();
            while let Some(selection) = pending.pop() {
                match &selection.node {
                    Selection::Field(field) => {
                        if !matches!(
                            field.node.name.node.as_str(),
                            "setClipboard"
                                | "deleteClipboardItems"
                                | "advanceAudioSelection"
                                | "saveMediaPlayProgress"
                                | "sendChatItems"
                                | "forwardChatItems"
                                | "sendShare"
                                | "sendFolderShare"
                                | "renameFile"
                                | "copyFile"
                                | "moveFile"
                                | "copyFileToDirectory"
                                | "createFileTask"
                                | "cacheSharedFile"
                                | "cacheZipEntry"
                                | "cacheAudioCover"
                                | "exportAppLogs"
                                | "clearAppCache"
                                | "syncFeeds"
                                | "acquireDiscovery"
                                | "releaseDiscovery"
                                | "deleteMediaItems"
                                | "trashMediaItems"
                                | "restoreMediaItems"
                                | "moveMediaItems"
                                | "broadcastImageEditorUpdate"
                                | "adjustPomodoro"
                                | "startPomodoro"
                                | "pausePomodoro"
                                | "stopPomodoro"
                        ) {
                            return true;
                        }
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
            false
        })
}

#[cfg(feature = "core")]
#[derive(Clone)]
pub struct GraphqlExecutor {
    state: ServerState,
}
#[cfg(feature = "core")]
impl GraphqlExecutor {
    pub(crate) fn new(state: ServerState) -> Self {
        Self { state }
    }
    pub async fn execute(&self, request: Request) -> Response {
        execute_graphql(
            &self.state.public,
            request,
            RequestContext::local(self.state.clone()),
        )
        .await
    }
}

#[cfg(feature = "core")]
pub(crate) fn subscription_stream(
    state: ServerState,
    request: Request,
    context: RequestContext,
) -> futures_util::stream::BoxStream<'static, Response> {
    use futures_util::StreamExt;
    let valid = (|| {
        let document = async_graphql::parser::parse_query(&request.query).ok()?;
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
        (operations.len() == 1
            && operations[0].1.node.ty == async_graphql::parser::types::OperationType::Subscription)
            .then_some(())
    })()
    .is_some();
    let request = if valid {
        context.prepare(request)
    } else {
        Err(Response::from_errors(vec![
            async_graphql::ServerError::new("Subscription document required", None),
        ]))
    };
    match request {
        Ok(request) => state.public.execute_stream(request),
        Err(response) => futures_util::stream::once(async move { response }).boxed(),
    }
}

#[cfg(all(test, feature = "core"))]
#[path = "../../tests/unit/graphql/executor.rs"]
mod tests;
