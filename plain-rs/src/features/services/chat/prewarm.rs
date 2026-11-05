use crate::features::chat::attachments::prewarm;
use crate::features::chat::attachments::prewarm::Advertisement;
use crate::features::chat::attachments::prewarm::Capabilities;
use crate::features::chat::attachments::prewarm::Driver;
use crate::features::services::peers::peer_transport;
use crate::features::services::peers::peer_wire;
use crate::runtime::Runtime as CoreRuntime;
use anyhow::{Result, bail};
use serde_json::{Value, json};

struct Native<'a>(&'a CoreRuntime);
impl Driver for Native<'_> {
    async fn capabilities(&self) -> Result<Capabilities> {
        let supported = peer_transport::capabilities(&self.0.host)
            .await
            .map_err(anyhow::Error::msg)?;
        Ok(Capabilities {
            ble_ready: supported
                .contains(&crate::infrastructure::transport::router::TransportType::Ble),
            aware_supported: supported
                .contains(&crate::infrastructure::transport::router::TransportType::Aware),
        })
    }
    async fn discover(&self, client_id: &str) -> Result<Option<Advertisement>> {
        let owner = self
            .0
            .host
            .generation()
            .ok_or_else(|| anyhow::anyhow!("BLE host unavailable"))?;
        let found: Option<String> = serde_json::from_value(
            self.0
                .host
                .call_current(
                    owner,
                    "peerTransportPrewarmScan",
                    json!({"clientId":client_id,"timeoutMs":15000}),
                )
                .await
                .map_err(anyhow::Error::msg)?,
        )?;
        let Some(found) = found else {
            return Ok(None);
        };
        anyhow::ensure!(found == client_id, "Peer prewarm advertisement mismatch");
        let Some(payload) =
            crate::features::services::discovery::ble_scan::read_payload(self.0, owner, client_id)
                .await
                .map_err(anyhow::Error::msg)?
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
    async fn start_aware(&self, peer_id: &str) -> Result<bool> {
        let prepared = peer_wire::prepare(
            &self.0.db,
            &self.0.prefs,
            peer_id,
            peer_wire::Operation::StartAware,
        )?;
        if !prepared.peer.is_paired() {
            bail!("Peer no longer paired")
        }
        let response = peer_transport::send(
            self.0,
            &prepared.peer,
            &prepared.channel_id,
            &plain_libs::utils::base64::base64_decode(&prepared.key),
            &prepared.body,
        )
        .await
        .map_err(anyhow::Error::msg)?;
        started(&response)
    }
}
fn started(response: &Value) -> Result<bool> {
    if response
        .get("errors")
        .is_some_and(|v| !v.is_null() && !v.as_array().is_some_and(Vec::is_empty))
    {
        bail!("Peer startAware returned GraphQL errors");
    }
    response["data"]["startAware"]
        .as_bool()
        .ok_or_else(|| anyhow::anyhow!("Invalid peer startAware receipt"))
}
fn conversation_peer(
    filter: &crate::infrastructure::db::chat_store::messages::Filter,
) -> Option<&str> {
    if filter.offset != 0
        || filter.count_only
        || filter.latest
        || !filter.text.is_empty()
        || filter.channel.is_some()
        || filter.limit.is_some_and(|limit| limit <= 0)
    {
        return None;
    }
    filter
        .peer
        .as_deref()
        .filter(|id| !id.trim().is_empty() && *id != "local")
}

pub fn prepare_page(state: &CoreRuntime, target: &str, text: &str, offset: i32, limit: i32) {
    let Ok((peer, channel)) = crate::features::chat::messages::target::parse(target) else {
        return;
    };
    prepare_conversation(
        state,
        &crate::infrastructure::db::chat_store::messages::Filter {
            peer: channel.is_empty().then_some(peer),
            channel: (!channel.is_empty()).then_some(channel),
            text: text.to_owned(),
            offset: offset.into(),
            limit: Some(limit.into()),
            descending: true,
            latest: false,
            count_only: false,
        },
    );
}

pub fn prepare_conversation(
    state: &CoreRuntime,
    filter: &crate::infrastructure::db::chat_store::messages::Filter,
) {
    let Some(id) = conversation_peer(filter) else {
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
        _ = stop.changed() => Err(anyhow::anyhow!("Server stopped")),
        result = prewarm::run(&state.db, &state.prewarmer, id, &native) => result,
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/chat/prewarm.rs"]
mod tests;
