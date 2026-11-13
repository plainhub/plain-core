use crate::graphql::executor::GraphqlExecutor;
use crate::managed_peers::request_error as peer_error;
use crate::server::ServerState;
use plain_graphql::{Request, Response};
use plain_rs::infrastructure::host::request_scope::NativeRequestScope;
use plain_rs::infrastructure::protocols::graphql::PeerSubscriptionTarget;

#[derive(Clone)]
pub struct NativeGraphqlExecutor {
    executor: GraphqlExecutor,
    state: ServerState,
    client_id: String,
}

impl NativeGraphqlExecutor {
    pub(crate) fn new(state: ServerState, client_id: String) -> Self {
        Self {
            executor: GraphqlExecutor::new(state.clone()),
            state,
            client_id,
        }
    }

    pub fn open_subscriptions(&self, scope: NativeRequestScope) -> Result<u64, String> {
        self.state
            .subscription_sessions
            .open(
                self.state.clone(),
                crate::graphql::executor::RequestContext::local(self.state.clone()),
                self.client_id.clone(),
                None,
                scope,
            )
            .map(|session| session.id)
    }
    pub fn open_peer_subscriptions(
        &self,
        target: PeerSubscriptionTarget,
        scope: NativeRequestScope,
    ) -> Result<u64, String> {
        self.state
            .subscription_sessions
            .open_peer(self.state.clone(), target, scope)
    }
    pub async fn open_managed_peer_subscriptions(
        &self,
        peer_id: &str,
        scope: NativeRequestScope,
    ) -> Result<u64, String> {
        let mut stop = self.state.stop.clone();
        tokio::select! {
            biased;
            _ = scope.wait_closed() => Err("Native request scope closed".into()),
            _ = stop.wait_for(|value| *value) => Err("Server stopped".into()),
            result = async {
                let route = self.state.managed_resources.route(&self.state, peer_id).await.map_err(peer_error)?;
                route.check_current(&self.state).map_err(peer_error)?;
                self.state.subscription_sessions.open_peer_with_pin(
                    self.state.clone(),
                    PeerSubscriptionTarget { peer_id: route.peer.id.clone(), public_key: route.peer.public_key.clone() },
                    scope.clone(),
                    Some(route.pin),
                )
            } => result,
        }
    }

    pub async fn execute_peer_scoped(
        &self,
        peer_id: &str,
        mut request: Request,
        scope: NativeRequestScope,
        timeout: std::time::Duration,
    ) -> Result<Response, String> {
        let deadline = tokio::time::Instant::now() + timeout;
        request.extensions.remove("nativeScope");
        request.extensions.remove("nativeDeadlineMs");
        let request =
            plain_libs::serde_json::to_string(&request).map_err(|error| error.to_string())?;
        if request.len() > plain_rs::features::services::peers::managed_resources::GRAPHQL_LIMIT {
            return Err("GraphQL request too large".into());
        }
        let mut stop = self.state.stop.clone();
        tokio::select! {
            biased;
            _ = scope.wait_closed() => Err("Native request scope closed".into()),
            _ = stop.wait_for(|value| *value) => Err("Server stopped".into()),
            _ = tokio::time::sleep_until(deadline) => Err("connection_timeout".into()),
            result = async {
                let route = self.state.managed_resources.route(&self.state, peer_id).await?;
                let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                let response = route.exchange(&self.state, &request, Some(remaining)).await?;
                plain_libs::serde_json::from_slice(&response).map_err(plain_libs::anyhow::Error::from)
            } => result.map_err(peer_error),
        }
    }

    pub fn subscription_session(
        &self,
        id: u64,
    ) -> Result<crate::graphql::subscriptions::session::SubscriptionSession, String> {
        self.state.subscription_sessions.get(id)
    }
    pub fn close_subscriptions(&self, id: u64) {
        self.state.subscription_sessions.close(id);
    }

    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    pub async fn execute(&self, request: Request) -> Response {
        self.executor.execute(request).await
    }
}

impl NativeGraphqlExecutor {
    pub async fn execute_scoped(&self, request: Request, scope: NativeRequestScope) -> Response {
        let mut closed = scope.cancellation();
        let rejected = || {
            Response::from_errors(vec![plain_graphql::ServerError::new(
                "Native request scope closed",
                None,
            )])
        };
        if *closed.borrow() {
            return rejected();
        }
        tokio::select! {
            biased;
            _ = closed.changed() => rejected(),
            result = self.execute(request.data(scope)) => result,
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-server/unit/native/graphql.rs"]
mod tests;
