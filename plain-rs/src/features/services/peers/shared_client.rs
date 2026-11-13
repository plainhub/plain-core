use crate::features::chat::messages::share_card;
use crate::features::chat::messages::share_card::Card;
use crate::features::files::shares::client::Info;
use crate::features::files::shares::client::Link;
use crate::runtime::Runtime as CoreRuntime;
use plain_libs::anyhow::{Result, ensure};
use plain_libs::serde_json::{Value, json};
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
fn candidates(state: &CoreRuntime, card: &Card, discovered: bool) -> Result<Vec<Link>> {
    let mut links = vec![];
    let mut seen = HashSet::new();
    if state
        .prefs
        .get::<String>(crate::features::prefs::pref_keys::CLIENT_ID)?
        .as_deref()
        == Some(&card.peer_info.id)
    {
        let port = state.prefs.get::<u16>("https_port")?.unwrap_or(8443);
        add(&mut links, &mut seen, "127.0.0.1", port, card);
        for local in crate::infrastructure::transport::lan::local_interfaces() {
            add(&mut links, &mut seen, &local.ip.to_string(), port, card);
        }
    }
    add(
        &mut links,
        &mut seen,
        &card.peer_info.ip,
        card.peer_info.port,
        card,
    );
    if let Some(peer) =
        crate::infrastructure::db::chat_store::peers::get(&state.db, &card.peer_info.id)?
    {
        if let Some(port) = peer.port {
            add(&mut links, &mut seen, &peer.ip, port, card);
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
        for key in ["ips", "ip6s"] {
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
    let bytes =
        crate::features::services::peers::peer_lan::limited(response, 4 * 1024 * 1024).await?;
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
                    info.expires_at.map(|instant| instant.0.timestamp_millis()),
                )?;
                if let Some(row) = updated {
                    let _ = state
                        .events
                        .send(crate::infrastructure::events::WsEvent::broadcast(
                            crate::features::chat::events::WS_MESSAGE_UPDATED,
                            json!([crate::features::chat::service::chat_to_json(&row)]).to_string(),
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
        _ = stop.changed() => Err(plain_libs::anyhow::anyhow!("Core stopped")),
        result = tokio::time::timeout(Duration::from_secs(120), work) =>
            result.unwrap_or_else(|_| Err(plain_libs::anyhow::anyhow!("Share request timed out"))),
    }
}

pub async fn own_link(state: &CoreRuntime, id: &str) -> Result<Link> {
    request(state, async {
        let service =
            crate::features::files::shares::Service::new(state.db.clone(), state.prefs.clone());
        let token = service.token(id)?;
        let host = {
            crate::infrastructure::transport::lan::local_interfaces()
                .first()
                .map(|interface| interface.ip.to_string())
                .unwrap_or_else(|| "127.0.0.1".into())
        };
        Link::new(
            &host,
            state
                .public_ports()
                .await
                .map(|ports| ports.1)
                .unwrap_or(state.prefs.get::<u16>("https_port")?.unwrap_or(8443)),
            id,
            &token,
        )
    })
    .await
}

pub async fn browse(
    state: &CoreRuntime,
    message_id: &str,
    expected: &Card,
    relative_path: Option<&str>,
) -> Result<Option<BrowseResult>> {
    request(
        state,
        browse_current(state, message_id, expected, relative_path),
    )
    .await
}

pub async fn files(
    state: &CoreRuntime,
    mut input: plain_libs::filesystem::browse::Request,
) -> Result<plain_libs::serde_json::Value> {
    use plain_libs::filesystem::record::FileRecord;
    #[derive(serde::Serialize)]
    struct File {
        #[serde(flatten)]
        record: FileRecord,
        url: String,
    }
    #[derive(serde::Serialize)]
    struct Page {
        count: usize,
        items: Vec<File>,
    }
    let (link, path) = Link::parse(&input.root)?;
    input.root = format!("/{path}");
    let plan = input.plan()?;
    let (info, link) =
        if let Some((id, card)) = share_card::find(&state.db, &link.shared_id, &link.token)? {
            let result = browse(state, &id, &card, Some(&plan.root))
                .await?
                .ok_or_else(|| plain_libs::anyhow::anyhow!("Share unavailable"))?;
            (result.info, result.link)
        } else {
            let info = request(state, fetch(state, &link, Some(&plan.root))).await?;
            (info, link)
        };
    let mut records = vec![];
    for entry in info.entries {
        ensure!(entry.size >= 0, "Invalid shared file size");
        records.push(FileRecord {
            name: entry.name,
            path: link.directory_url(&entry.relative_path)?,
            permission: String::new(),
            file_id: String::new(),
            created_at: entry.created_at.map(|time| time.0.timestamp_millis()),
            updated_at: entry.updated_at.0.timestamp_millis(),
            size: entry.size,
            is_dir: entry.is_dir,
            child_count: 0,
        });
    }
    let page = plan.records_page(records);
    let items = page
        .items
        .into_iter()
        .map(|record| -> Result<_> {
            let (_, path) = Link::parse(&record.path)?;
            let url = format!(
                "{}&action=download",
                link.file_url(&info.url_token, &path, false)?
            );
            Ok(File { record, url })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(plain_libs::serde_json::to_value(Page {
        count: page.count,
        items,
    })?)
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/peers/shared_client.rs"]
mod tests;
