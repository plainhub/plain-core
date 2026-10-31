use std::sync::{Arc, Mutex};

pub(crate) struct ClientConnections {
    pub(crate) host: Arc<plain_rs::host::bridge::Host>,
    pub(crate) login_attempts: Arc<Mutex<crate::http::auth::ws_login::LoginAttempts>>,
    pub(crate) subscriptions: Arc<crate::graphql::subscriptions::session::Sessions>,
    pub(crate) sockets: Arc<crate::http::websocket::runtime::Runtime>,
}
impl plain_rs::host::client_connections::ClientConnections for ClientConnections {
    fn close(&self, client: Option<&str>) {
        self.sockets.close_client(&self.subscriptions, client);
    }
    fn revoke(&self, client: &str) -> Result<(), String> {
        let mut attempts = self
            .login_attempts
            .lock()
            .map_err(|_| "Login state unavailable")?;
        for request in attempts.cancel_client(client) {
            attempts.notify_host(
                self.host.clone(),
                self.login_attempts.clone(),
                "systemWebLoginClosed",
                serde_json::json!({"requestId":request}),
            );
        }
        drop(attempts);
        self.close(Some(client));
        Ok(())
    }
    fn contains(&self, client: &str) -> bool {
        self.subscriptions.snapshot().iter().any(|id| id == client)
    }
}
