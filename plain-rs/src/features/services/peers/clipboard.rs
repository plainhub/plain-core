use crate::features::access::AccessFeature;
use crate::features::capability::Capability;
use crate::features::clipboard;
use crate::features::services::peers::management::{Identity, current_peer};
use crate::infrastructure::db::DPeer;
use crate::infrastructure::db::chat_store::peers;
use crate::runtime::Runtime;
use plain_libs::anyhow::{Result, ensure};
use plain_libs::serde_json::json;
use tokio::sync::watch;

#[derive(serde::Deserialize)]
pub(crate) struct Change {
    pub text: String,
    pub sensitive: bool,
}

/// The authenticated login must also be a device with a current outbound
/// management grant. Ordinary browser/API users and chat-only peers cannot
/// receive automatic clipboard payloads, even if they can query history.
pub fn can_publish(state: &Runtime, peer_id: &str) -> Result<bool> {
    if !clipboard::sync::direction(&state.prefs, peer_id)?.sends()
        || !state
            .db
            .session_get(peer_id)?
            .is_some_and(|session| session.r#type == crate::features::sessions::SessionType::Login)
    {
        return Ok(false);
    }
    Ok(peers::get(&state.db, peer_id)?.is_some_and(|peer| is_managed(&peer)))
}

fn is_managed(peer: &DPeer) -> bool {
    peer.is_paired() && crate::features::sessions::token_key(&peer.token).is_ok()
}

pub(super) fn may_receive(state: &Runtime, peer_id: &str) -> Result<bool> {
    Ok(
        clipboard::sync::direction(&state.prefs, peer_id)?.receives()
            && crate::features::services::device::access::is_enabled(
                &state.prefs,
                &[AccessFeature::Clipboard],
            )
            && peers::get(&state.db, peer_id)?.is_some_and(|peer| is_managed(&peer)),
    )
}

pub(super) async fn receive(
    state: &Runtime,
    peer_id: &str,
    identity: &Identity,
    canceled: &watch::Receiver<bool>,
    change: Change,
) -> Result<()> {
    if change.sensitive || !may_receive(state, peer_id)? {
        return Ok(());
    }
    clipboard::validate_text(&change.text).map_err(plain_libs::anyhow::Error::msg)?;
    let Some(host_generation) = state.host.generation() else {
        return Ok(());
    };
    let capabilities = crate::features::services::capabilities::snapshot(&state.host).await?;
    if !capabilities.contains(&Capability::Clipboard)
        || !crate::features::services::device::access::granted(
            &state.host,
            &[AccessFeature::Clipboard],
        )
        .await?
        || !current_peer(state, peer_id, identity, canceled).await?
        || !may_receive(state, peer_id)?
    {
        return Ok(());
    }
    let _receipt = state.clipboard_writes.begin(&change.text);
    let written = state
        .host
        .call_current(
            host_generation,
            "systemSetClipboard",
            json!({"text":&change.text,"label":"","sensitive":false}),
        )
        .await
        .map_err(plain_libs::anyhow::Error::msg)?;
    ensure!(written.as_bool() == Some(true), "Clipboard write failed");
    crate::features::services::clipboard::record_write(
        &state.db,
        &state.host,
        &change.text,
        peer_id,
        "",
        false,
    )?;
    Ok(())
}
