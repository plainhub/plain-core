use crate::features::chat::attachments::prewarm;
use crate::features::chat::attachments::prewarm::Advertisement;
use crate::features::chat::attachments::prewarm::Capabilities;
use crate::features::chat::attachments::prewarm::Driver;
use crate::features::services::peers::peer_transport;
use crate::runtime::Runtime as CoreRuntime;
use plain_libs::anyhow::{Result, bail};
use plain_libs::serde_json::json;

struct Native<'a>(&'a CoreRuntime);
impl Driver for Native<'_> {
    async fn capabilities(&self) -> Result<Capabilities> {
        let supported = peer_transport::capabilities(&self.0.host)
            .await
            .map_err(plain_libs::anyhow::Error::msg)?;
        Ok(Capabilities {
            ble_ready: supported
                .contains(&crate::infrastructure::transport::router::TransportType::Ble),
        })
    }
    async fn discover(&self, client_id: &str) -> Result<Option<Advertisement>> {
        let owner = self
            .0
            .host
            .generation()
            .ok_or_else(|| plain_libs::anyhow::anyhow!("BLE host unavailable"))?;
        let found: Option<String> = plain_libs::serde_json::from_value(
            self.0
                .host
                .call_current(
                    owner,
                    "peerTransportPrewarmScan",
                    json!({"clientId":client_id,"timeoutMs":15000}),
                )
                .await
                .map_err(plain_libs::anyhow::Error::msg)?,
        )?;
        let Some(found) = found else {
            return Ok(None);
        };
        plain_libs::anyhow::ensure!(found == client_id, "Peer prewarm advertisement mismatch");
        let Some(payload) =
            crate::features::services::discovery::ble_scan::read_payload(self.0, owner, client_id)
                .await
                .map_err(plain_libs::anyhow::Error::msg)?
        else {
            return Ok(None);
        };
        let reply = crate::features::discovery::nearby_wire::discover_reply(&payload, client_id)?;
        Ok(Some(Advertisement {
            client_id: reply.id,
            aware_supported: reply.aware_supported,
            aware_running: reply.aware_running,
        }))
    }
}
pub fn prepare_conversation(
    state: &CoreRuntime,
    filter: &crate::infrastructure::db::chat_store::messages::Filter,
    offset: i32,
    limit: i32,
) {
    if offset != 0 || limit <= 0 || !filter.text.is_empty() || filter.channel.is_some() {
        return;
    }
    let Some(id) = filter
        .peer
        .as_deref()
        .filter(|id| !id.trim().is_empty() && *id != "local")
    else {
        return;
    };
    if *state.stop.borrow() {
        return;
    }
    let id = id.to_owned();
    let state = state.clone();
    tokio::spawn(async move {
        if let Err(error) = execute(&state, &id).await {
            log::warn!("Peer transport preparation failed: {error}");
        }
    });
}

async fn execute(state: &CoreRuntime, id: &str) -> Result<Option<Advertisement>> {
    let mut stop = state.stop.clone();
    if *stop.borrow() {
        bail!("Server stopped");
    }
    let native = Native(state);
    tokio::select! {
        _ = stop.changed() => Err(plain_libs::anyhow::anyhow!("Server stopped")),
        result = prewarm::run(&state.db, &state.prewarmer, id, &native) => result,
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/chat/prewarm.rs"]
mod tests;
