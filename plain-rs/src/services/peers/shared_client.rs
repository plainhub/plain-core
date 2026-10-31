use anyhow::{Result, ensure};
use plain_rs::chat::messages::share_card;
use plain_rs::chat::messages::share_card::Card;
use plain_rs::files::shares::client::Info;
use plain_rs::files::shares::client::Link;
use plain_rs::runtime::Runtime as CoreRuntime;
use serde_json::{Value, json};
use std::{collections::HashSet, time::Duration};
fn add(
    links: &mut Vec<Link>,
    seen: &mut HashSet<(String, u16)>,
    host: &str,
    port: u16,
    card: &Card,
) {
    if let Ok(link) = Link::new(host, port, &card.share_id, &card.url_token) {
        if seen.insert((link.host.clone(), port)) {
            links.push(link);
        }
    }
}
fn add_addresses(
    links: &mut Vec<Link>,
    seen: &mut HashSet<(String, u16)>,
    ips: &str,
    port: u16,
    card: &Card,
) {
    let mut ips: Vec<String> = ips
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    let best = plain_rs::transport::lan::best(&ips, &plain_rs::transport::lan::local_interfaces());
    if !best.is_empty() {
        add(links, seen, &best, port, card);
    }
    for ip in ips.drain(..) {
        add(links, seen, &ip, port, card);
    }
}
fn candidates(state: &CoreRuntime, card: &Card, discovered: bool) -> Result<Vec<Link>> {
    let mut links = vec![];
    let mut seen = HashSet::new();
    if state
        .prefs
        .get::<String>(plain_rs::prefs::pref_keys::CLIENT_ID)?
        .as_deref()
        == Some(&card.peer_info.id)
    {
        let port = state.prefs.get::<u16>("https_port")?.unwrap_or(8443);
        add(&mut links, &mut seen, "127.0.0.1", port, card);
        for local in plain_rs::transport::lan::local_interfaces() {
            add(&mut links, &mut seen, &local.ip.to_string(), port, card);
        }
    }
    add_addresses(
        &mut links,
        &mut seen,
        &card.peer_info.ip,
        card.peer_info.port,
        card,
    );
    if let Some(peer) = plain_rs::db::chat_store::peers::get(&state.db, &card.peer_info.id)? {
        if let Some(port) = peer.port {
            add_addresses(&mut links, &mut seen, &peer.ip, port, card);
        }
    }
    if discovered {
        discovery_candidates(&mut links, &mut seen, &state.mdns.snapshot(), card);
    }
    Ok(links)
}
fn discovery_candidates(
    links: &mut Vec<Link>,
    seen: &mut HashSet<(String, u16)>,
    snapshot: &Value,
    card: &Card,
) {
    for service in snapshot["services"].as_array().into_iter().flatten() {
        if service["complete"] != true
            || !service["txtRecords"].as_array().is_some_and(|txt| {
                txt.iter()
                    .any(|entry| entry.as_str() == Some(&format!("id={}", card.peer_info.id)))
            })
        {
            continue;
        }
        let Some(port) = service["port"]
            .as_u64()
            .and_then(|port| u16::try_from(port).ok())
        else {
            continue;
        };
        for key in ["ips", "ipv6"] {
            for host in service[key]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                add(links, seen, host, port, card);
            }
        }
    }
}
pub async fn fetch(state: &CoreRuntime, link: &Link, path: Option<&str>) -> Result<Info> {
    let _permit = state.lan.capacity.clone().acquire_owned().await?;
    let request = link.request(path)?;

    let response = state
        .lan
        .client
        .post(link.url("/guest_graphql")?)
        .timeout(Duration::from_secs(10))
        .header("content-type", "application/octet-stream")
        .header("c-id", &link.shared_id)
        .body(request.clone())
        .send()
        .await?;
    ensure!(
        response.status().is_success(),
        "Shared HTTP {}",
        response.status()
    );
    let bytes = plain_rs::services::peers::peer_lan::limited(response, 4 * 1024 * 1024).await?;
    link.response(&request, &bytes)
}
async fn browse_current(
    state: &CoreRuntime,
    id: &str,
    expected: &Card,
    path: Option<&str>,
) -> Result<Option<BrowseResult>> {
    let original = share_card::current(&state.db, id, expected)?;
    Link::new("127.0.0.1", 443, &expected.share_id, &expected.url_token)?;
    for round in 0..=12 {
        ensure!(
            share_card::current(&state.db, id, expected)? == original,
            "Share card changed"
        );
        if round > 0 {
            if (round - 1) % 4 == 0 {
                state.mdns.browse_resident();
            }
            tokio::time::sleep(Duration::from_millis(700)).await;
        }
        for link in candidates(state, expected, round > 0)? {
            ensure!(
                share_card::current(&state.db, id, expected)? == original,
                "Share card changed"
            );
            if let Ok(info) = fetch(state, &link, path).await {
                ensure!(
                    share_card::current(&state.db, id, expected)? == original,
                    "Share card changed"
                );
                let (card, updated) = share_card::refresh(
                    &state.db,
                    id,
                    expected,
                    &original,
                    &link.host,
                    link.port,
                    &info.name,
                    info.expires_at,
                )?;
                if let Some(row) = updated {
                    let _ = state.events.send(plain_rs::events::WsEvent::broadcast(
                        plain_rs::chat::events::WS_MESSAGE_UPDATED,
                        json!([plain_rs::chat::service::chat_to_json(&row)]).to_string(),
                    ));
                }
                return Ok(Some(BrowseResult { info, link, card }));
            }
        }
    }
    Ok(None)
}
pub struct BrowseResult {
    pub info: Info,
    pub link: Link,
    pub card: Card,
}

async fn request<T>(
    state: &CoreRuntime,
    work: impl std::future::Future<Output = Result<T>>,
) -> Result<T> {
    let mut stop = state.stop.clone();
    ensure!(!*stop.borrow(), "Core stopped");
    tokio::select! {
        _ = stop.changed() => Err(anyhow::anyhow!("Core stopped")),
        result = tokio::time::timeout(Duration::from_secs(120), work) =>
            result.unwrap_or_else(|_| Err(anyhow::anyhow!("Share request timed out"))),
    }
}

pub async fn own_link(state: &CoreRuntime, id: &str, host: Option<String>) -> Result<Link> {
    request(state, async {
        let service = plain_rs::files::shares::Service::new(state.db.clone(), state.prefs.clone());
        let token = service.token(id)?;
        let host = host.unwrap_or_else(|| {
            plain_rs::transport::lan::local_interfaces()
                .first()
                .map(|interface| interface.ip.to_string())
                .unwrap_or_else(|| "127.0.0.1".into())
        });
        Link::new(
            &host,
            state.prefs.get::<u16>("https_port")?.unwrap_or(8443),
            id,
            &token,
        )
    })
    .await
}

pub async fn initial_link(state: &CoreRuntime, message_id: &str, expected: &Card) -> Result<Link> {
    request(state, async {
        share_card::current(&state.db, message_id, expected)?;
        candidates(state, expected, false)?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Share address unavailable"))
    })
    .await
}

pub async fn browse(
    state: &CoreRuntime,
    message_id: &str,
    expected: &Card,
    virtual_path: Option<&str>,
) -> Result<Option<BrowseResult>> {
    request(
        state,
        browse_current(state, message_id, expected, virtual_path),
    )
    .await
}

pub async fn cache_file(
    state: &CoreRuntime,
    cache: &plain_rs::services::files::remote_cache::RemoteCache,
    link: Link,
    url_token: String,
    entry: plain_rs::files::shares::client::File,
    thumbnail: bool,
) -> Result<String> {
    ensure!(!entry.is_dir, "Shared preview requires a file");
    let mut url = link.file_url(&url_token, &entry.virtual_path, false)?;
    let (name, key) = if thumbnail {
        url.push_str("&w=200&h=200&cc=false");
        ("thumbnail".to_owned(), Some(json!({"host":link.host,"port":link.port,"shareId":link.shared_id,"token":link.token,"urlToken":url_token,"path":entry.virtual_path,"size":entry.size,"centerCrop":false}).to_string()))
    } else {
        (entry.name, None)
    };
    cache
        .download(
            &url,
            &name,
            key.as_deref(),
            state.stop.clone(),
            Some(&state.lan.client),
        )
        .await
        .map_err(anyhow::Error::msg)
}

#[cfg(test)]
#[path = "../../../tests/unit/services/peers/shared_client.rs"]
mod tests;
