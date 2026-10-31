use crate::runtime::Runtime as CoreRuntime;
use serde_json::json;
use std::sync::Arc;
pub struct Runtime {
    pub connections: plain_rs::peers::status::Connections,
    pub outgoing: Arc<plain_rs::services::peers::status_outgoing::Outgoing>,
    pub control: tokio::sync::Mutex<()>,
    pub public_active: std::sync::atomic::AtomicBool,
}
impl Runtime {
    pub fn snapshot(&self, db: &plain_rs::db::Db) -> serde_json::Value {
        let mut row = self.connections.snapshot(db);
        row["outgoing"] = self.outgoing.snapshot();
        row
    }
}
impl Default for Runtime {
    fn default() -> Self {
        Self {
            connections: Default::default(),
            control: tokio::sync::Mutex::new(()),
            public_active: std::sync::atomic::AtomicBool::new(false),
            outgoing: Arc::new(plain_rs::services::peers::status_outgoing::Outgoing::new()),
        }
    }
}
pub fn emit(state: &CoreRuntime, id: &str, online: bool) {
    if !id.is_empty() {
        let _ = state.events.send(plain_rs::events::WsEvent::broadcast(
            plain_rs::chat::events::WS_PEER_STATUS_UPDATED,
            json!({"id":id,"online":online}).to_string(),
        ));
    }
    let _=state.events.send(plain_rs::events::WsEvent::broadcast("PEER_CONNECTIONS_UPDATED",json!({"id":id,"online":online,"runtimeId":state.peer_status.snapshot(&state.db)["runtimeId"]}).to_string()));
}
pub struct ConnectionGuard {
    pub state: CoreRuntime,
    pub lease: String,
}
impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        if let Some((peer, online)) = self
            .state
            .peer_status
            .connections
            .close(&self.state.db, &self.lease)
        {
            emit(&self.state, &peer, online);
        }
    }
}
