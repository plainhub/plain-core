use anyhow::{Result, bail};
use plain_rs::chat::attachments::prewarm;
use plain_rs::chat::attachments::prewarm::Advertisement;
use plain_rs::chat::attachments::prewarm::Capabilities;
use plain_rs::chat::attachments::prewarm::Driver;
use plain_rs::runtime::Runtime as CoreRuntime;
use plain_rs::services::peers::peer_transport;
use plain_rs::services::peers::peer_wire;
use serde_json::{Value, json};

struct Native<'a>(&'a CoreRuntime);
impl Driver for Native<'_> {
    async fn capabilities(&self) -> Result<Capabilities> {
        let supported = peer_transport::capabilities(&self.0.host)
            .await
            .map_err(anyhow::Error::msg)?;
        Ok(Capabilities {
            ble_ready: supported.contains(&plain_rs::transport::router::TransportType::Ble),
            aware_supported: supported.contains(&plain_rs::transport::router::TransportType::Aware),
        })
    }
    async fn scan(&self, short_id: &str) -> Result<Option<Advertisement>> {
        Ok(serde_json::from_value(
            self.0
                .host
                .call(
                    "peerTransportPrewarmScan",
                    json!({"shortId":short_id,"timeoutMs":15000}),
                )
                .await
                .map_err(anyhow::Error::msg)?,
        )?)
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
            &plain_rs::utils::base64::base64_decode(&prepared.key),
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
fn conversation_peer(filter: &plain_rs::db::chat_store::messages::Filter) -> Option<&str> {
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
    let Ok((peer, channel)) = plain_rs::chat::messages::target::parse(target) else {
        return;
    };
    prepare_conversation(
        state,
        &plain_rs::db::chat_store::messages::Filter {
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
    filter: &plain_rs::db::chat_store::messages::Filter,
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
#[path = "../../../tests/unit/services/chat/prewarm.rs"]
mod tests;
