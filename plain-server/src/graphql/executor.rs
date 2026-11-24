#[cfg(feature = "core")]
use crate::server::ServerState;
use plain_graphql::{ObjectType, Request, Response, Schema, SubscriptionType};
#[cfg(feature = "core")]
use plain_rs::infrastructure::events::WsEvent;

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

#[cfg(feature = "core")]
pub(crate) fn resource_credentials(
    ctx: &plain_graphql::Context<'_>,
) -> plain_graphql::Result<(plain_rs::features::files::resource_link::Scope, String)> {
    use plain_rs::features::files::resource_link::Scope;
    if ctx.data_opt::<RequestOrigin>() == Some(&RequestOrigin::Remote) {
        let id = ctx.data::<RequestClient>()?.client_id();
        let row = ctx
            .data::<std::sync::Arc<plain_rs::infrastructure::db::Db>>()?
            .session_get(id)?
            .ok_or_else(|| plain_graphql::Error::new("Session unavailable"))?;
        Ok((Scope::Session(row.client_id), row.url_token))
    } else {
        let state = ctx.data::<plain_rs::runtime::Runtime>()?;
        let client_id = state
            .native_client_id()
            .map_err(plain_graphql::Error::new)?;
        Ok((Scope::Session(client_id), state.token.to_string()))
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
                                plain_graphql::ServerError::new(error, None),
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

pub(crate) fn error_codes(mut response: Response) -> Response {
    for error in &mut response.errors {
        if error.extensions.as_ref().is_some_and(|extensions| extensions.get("code").is_some()) { continue; }
        let mut code = None;
        if let Some(source) = &error.source {
            macro_rules! classify {
                ($ty:ty) => {
                    if code.is_none() {
                        code = source.downcast_ref::<$ty>().map(|value| value.as_str())
                            .or_else(|| source.downcast_ref::<plain_libs::anyhow::Error>()
                                .and_then(<$ty>::from_error).map(|value| value.as_str()))
                            .or_else(|| source.downcast_ref::<plain_rs::infrastructure::error::DomainError>()
                                .and_then(|error| std::error::Error::source(error))
                                .and_then(|source| source.downcast_ref::<$ty>()).map(|value| value.as_str()));
                    }
                };
            }
            classify!(plain_rs::features::services::auth_error::AuthErrorCode);
            classify!(plain_rs::features::services::discovery::pairing_error::PairingErrorCode);
            classify!(plain_rs::features::access_error::AccessErrorCode);
            classify!(plain_rs::features::clipboard::error::ClipboardErrorCode);
            classify!(plain_rs::features::feeds::error::FeedOperationErrorCode);
            classify!(plain_rs::features::media::audio::error::AudioQueueErrorCode);
            classify!(plain_rs::features::prefs::error::PreferenceErrorCode);
            classify!(plain_rs::infrastructure::db::key_value_error::KeyValueErrorCode);
            classify!(plain_rs::features::services::notifications::error::NotificationErrorCode);
            classify!(plain_rs::infrastructure::platform::mount_error::MountErrorCode);
            classify!(plain_rs::features::services::disk_format_error::DiskFormatErrorCode);
            classify!(plain_rs::infrastructure::protocols::graphql::error::SubscriptionErrorCode);
            classify!(plain_rs::features::peers::error::PeerConnectionErrorCode);
            classify!(plain_rs::infrastructure::http_error::HttpErrorCode);
            classify!(plain_rs::infrastructure::native_error::NativeErrorCode);
            classify!(plain_rs::infrastructure::graphql_error::GraphqlErrorCode);
            classify!(plain_rs::features::files::archive_error::ArchiveErrorCode);
            classify!(plain_rs::features::files::share_access_error::ShareAccessErrorCode);
        }
        error.extensions.get_or_insert_with(Default::default).set("code", code.unwrap_or("INTERNAL_ERROR"));
    }
    response
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
        Err(response) => return error_codes(response),
    };
    let response = error_codes(schema.execute(request).await);
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

pub(crate) fn root_fields<'a>(
    document: &'a plain_graphql::parser::types::ExecutableDocument,
    operation: &'a plain_graphql::parser::types::OperationDefinition,
) -> std::collections::HashSet<(&'a str, &'a str)> {
    use plain_graphql::parser::types::Selection;
    let mut pending = operation
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
    names
}

pub(crate) fn content_changes(request: &Request) -> bool {
    use plain_graphql::parser::{
        parse_query,
        types::OperationType,
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
            root_fields(&document, &operation.node).into_iter().any(|(_, field)| {
                !matches!(field, "setClipboard"
                    | "deleteClipboardItems"
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
                    | "cacheZipEntry"
                    | "exportAppLogs"
                    | "clearAppCache"
                    | "syncFeeds"
                    | "startDiscovery"
                    | "stopDiscovery"
                    | "deleteMediaItems"
                    | "trashMediaItems"
                    | "restoreMediaItems"
                    | "moveMediaItems"
                    | "broadcastImageEditorUpdate"
                    | "setPomodoroRemainingTime"
                    | "startPomodoro"
                    | "pausePomodoro"
                    | "stopPomodoro"
                )
            })
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
        let document = plain_graphql::parser::parse_query(&request.query).ok()?;
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
            && operations[0].1.node.ty == plain_graphql::parser::types::OperationType::Subscription)
            .then_some(())
    })()
    .is_some();
    let request = if valid {
        context.prepare(request)
    } else {
        Err(Response::from_errors(vec![
            plain_graphql::ServerError::new("Subscription document required", None),
        ]))
    };
    match request {
        Ok(request) => state
            .public
            .execute_stream(request)
            .map(error_codes)
            .boxed(),
        Err(response) => futures_util::stream::once(async move { error_codes(response) }).boxed(),
    }
}

#[cfg(all(test, feature = "core"))]
#[path = "../../../tests/plain-server/unit/graphql/executor.rs"]
mod tests;
