use anyhow::{Result, ensure};
use futures_util::StreamExt;
use plain_rs::runtime::Runtime as CoreRuntime;
use plain_rs::services::peers::peer_transport::Pending;
use plain_rs::transport::router::Outcome;
use plain_rs::transport::router::TransportType;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

pub async fn run(
    state: &CoreRuntime,
    task: &plain_rs::chat::attachments::download_queue::Snapshot,
    path: &str,
    attempt: plain_rs::transport::router::Attempt,
) -> Result<()> {
    let peer = plain_rs::services::peers::peer_address::current_identity(
        &state.db,
        &task.peer.id,
        &task.peer,
    )?;
    let file_id = task.file["uri"]
        .as_str()
        .and_then(|s| s.strip_prefix("fsid:"))
        .ok_or_else(|| anyhow::anyhow!("Invalid remote attachment"))?;
    let mut available = vec![TransportType::Lan];
    available.extend(
        plain_rs::services::peers::peer_transport::capabilities(&state.host)
            .await
            .map_err(anyhow::Error::msg)?
            .into_iter()
            .filter(|kind| *kind != TransportType::Lan),
    );
    let current_peer =
        plain_rs::services::peers::peer_address::current_identity(&state.db, &peer.id, &peer)?;
    let step = state
        .transport
        .begin_with_attempt(&current_peer, &available, attempt)?;
    let ticket = step
        .ticket
        .ok_or_else(|| anyhow::anyhow!(step.error.unwrap_or_else(|| "No peer transport".into())))?;
    let mut pending = Pending {
        state,
        router: &state.transport,
        ticket,
    };
    let permit = state.lan.capacity.clone().acquire_owned().await?;
    let actor = state
        .prefs
        .get::<String>(plain_rs::prefs::pref_keys::CLIENT_ID)?
        .filter(|v| !v.is_empty())
        .ok_or_else(|| anyhow::anyhow!("Missing peer actor"))?;
    let mut file = open_ticket(path).await?;
    let mut bytes = 0;
    loop {
        plain_rs::services::peers::peer_transport::notify(state);
        let peer =
            plain_rs::services::peers::peer_address::current_identity(&state.db, &peer.id, &peer)?;
        let unavailable = match pending.ticket.transport {
            TransportType::Lan => {
                let url = plain_rs::services::peers::peer_address::file_url(&peer, file_id)?;
                let response = tokio::time::timeout(
                    Duration::from_secs(10),
                    state.lan.client.get(url).header("c-id", &actor).send(),
                )
                .await;
                match response {
                    Ok(Ok(response)) if response.status().is_success() => {
                        state
                            .transport
                            .finish(&pending.ticket, Outcome::Connected)?;
                        let mut chunks = response.bytes_stream();
                        while let Some(chunk) =
                            tokio::time::timeout(Duration::from_secs(30), chunks.next()).await?
                        {
                            write(state, task, &mut file, &mut bytes, &chunk?).await?;
                        }
                        None
                    }
                    Ok(Ok(response)) => Some(format!("Peer HTTP {}", response.status())),
                    Ok(Err(error)) => Some(error.to_string()),
                    Err(error) => Some(error.to_string()),
                }
            }
            TransportType::Aware => {
                let url = plain_rs::services::peers::peer_address::file_url(&peer, file_id)?;
                let url = reqwest::Url::parse(&url)?;
                let path = format!("{}?{}", url.path(), url.query().unwrap_or_default());
                let response = tokio::time::timeout(
                    Duration::from_secs(15),
                    plain_rs::services::peers::peer_sdk::aware(
                        state,
                        &peer,
                        "GET",
                        &path,
                        "",
                        vec![],
                    ),
                )
                .await;
                match response {
                    Ok(Ok(response)) if (200..300).contains(&response.status) => {
                        state
                            .transport
                            .finish(&pending.ticket, Outcome::Connected)?;
                        let _lease = response.lease;
                        let mut chunks =
                            plain_rs::services::peers::peer_lan::incoming_chunks(response.body);
                        while let Some(chunk) =
                            tokio::time::timeout(Duration::from_secs(30), chunks.next()).await?
                        {
                            write(state, task, &mut file, &mut bytes, &chunk?).await?;
                        }
                        None
                    }
                    Ok(Ok(response)) => Some(format!("Peer HTTP {}", response.status)),
                    Ok(Err(plain_rs::services::peers::peer_lan::Failure::Unavailable(error))) => {
                        Some(error)
                    }
                    Ok(Err(plain_rs::services::peers::peer_lan::Failure::Fatal(error))) => {
                        return Err(anyhow::anyhow!(error));
                    }
                    Err(error) => Some(error.to_string()),
                }
            }
            TransportType::Ble => {
                let mut offset = 0u64;
                loop {
                    plain_rs::services::peers::peer_address::current_identity(
                        &state.db, &peer.id, &peer,
                    )?;
                    let response = plain_rs::services::peers::peer_sdk::ble(
                        state,
                        &peer,
                        plain_rs::protocols::ble::Request::FileChunk {
                            client_id: plain_rs::services::peers::peer_sdk::ble_actor(state)
                                .map_err(|e| anyhow::anyhow!("{e:?}"))?,
                            file_id: file_id.into(),
                            offset,
                            length: 8192,
                        },
                    )
                    .await
                    .map_err(|e| anyhow::anyhow!("{e:?}"))?;
                    let (status, chunk) = plain_rs::protocols::ble::decode_response(&response)?;
                    ensure!(status == 200, "BLE file HTTP failure");
                    ensure!(chunk.len() <= 8192, "Invalid BLE chunk length");
                    write(state, task, &mut file, &mut bytes, &chunk).await?;
                    offset += chunk.len() as u64;
                    if chunk.len() < 8192 {
                        break;
                    }
                }
                state
                    .transport
                    .finish(&pending.ticket, Outcome::Connected)?;
                None
            }
        };
        match unavailable {
            None => break,
            Some(error) => {
                let step = state
                    .transport
                    .finish(&pending.ticket, Outcome::Unavailable { error })?;
                pending.ticket = step.ticket.ok_or_else(|| {
                    anyhow::anyhow!(step.error.unwrap_or_else(|| "Peer unavailable".into()))
                })?;
            }
        }
    }
    ensure!(bytes == task.total, "Attachment byte count mismatch");
    plain_rs::services::peers::peer_address::current_identity(&state.db, &peer.id, &peer)?;
    file.flush().await?;
    file.sync_all().await?;
    drop(file);
    drop(permit);
    Ok(())
}
async fn write(
    state: &CoreRuntime,
    task: &plain_rs::chat::attachments::download_queue::Snapshot,
    file: &mut tokio::fs::File,
    bytes: &mut u64,
    chunk: &[u8],
) -> Result<()> {
    *bytes = bytes
        .checked_add(chunk.len() as u64)
        .ok_or_else(|| anyhow::anyhow!("Attachment size overflow"))?;
    ensure!(*bytes <= task.total, "Attachment exceeds declared size");
    file.write_all(chunk).await?;
    ensure!(
        state
            .downloads
            .progress(&task.id, &task.generation, *bytes)?,
        "Attachment transfer expired"
    );
    Ok(())
}

async fn open_ticket(path: &str) -> std::io::Result<tokio::fs::File> {
    tokio::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(path)
        .await
}

#[cfg(test)]
#[path = "../../../tests/unit/services/peers/peer_download.rs"]
mod tests;
