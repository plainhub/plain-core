use anyhow::Result;
use plain_rs::chat::messages::share_card::{Card, Peer};
use plain_rs::files::shares::{Service, summary};
use plain_rs::runtime::Runtime as CoreRuntime;
use std::sync::Arc;

pub async fn content(state: &CoreRuntime, service: Arc<Service>, id: String) -> Result<String> {
    let row = state
        .db
        .share_get(&id)?
        .ok_or_else(|| anyhow::anyhow!("Share not found"))?;
    let roots = Service::roots(&row)?;
    let paths = roots
        .iter()
        .map(|root| root.real_path.clone())
        .collect::<Vec<_>>();
    state.file_platform.authorize_metadata_paths(&paths).await?;
    let (summary, token) = tokio::task::spawn_blocking(move || -> Result<_> {
        Ok((summary::summarize(&roots)?, service.token(&id)?))
    })
    .await??;
    let card = Card {
        share_id: row.id,
        url_token: token,
        peer_info: Peer {
            id: state.native_client_id().map_err(anyhow::Error::msg)?,
            ip: plain_rs::platform::network::host::lan_ipv4(&state.host)
                .await
                .map_err(anyhow::Error::msg)?,
            port: state.public_ports().await.map(|ports| ports.1).unwrap_or(0),
        },
        name: row.name,
        item_count: summary.item_count,
        total_size: summary.total_size,
        expires_at: row.expires_at.map(|value| value.parse()).transpose()?,
    };
    Ok(serde_json::json!({"type":"SHARE", "value":card}).to_string())
}
