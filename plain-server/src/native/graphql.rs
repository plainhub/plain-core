use crate::graphql::executor::GraphqlExecutor;
use crate::native::events::NativeEventStream;
use crate::server::ServerState;
use async_graphql::{Request, Response};
use plain_rs::{
    host::request_scope::NativeRequestScope, protocols::graphql::PeerSubscriptionTarget,
};

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

    pub fn subscribe_events(&self, scope: NativeRequestScope) -> Result<NativeEventStream, String> {
        NativeEventStream::scoped(self.state.clone(), self.client_id.clone(), scope)
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
            Response::from_errors(vec![async_graphql::ServerError::new(
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
#[path = "../../tests/unit/native/graphql.rs"]
mod tests;
