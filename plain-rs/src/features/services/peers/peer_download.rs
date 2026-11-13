use crate::features::services::peers::peer_transport::Pending;
use crate::infrastructure::transport::router::Outcome;
use crate::infrastructure::transport::router::TransportType;
use crate::runtime::Runtime as CoreRuntime;
use futures_util::StreamExt;
use plain_libs::anyhow::{Result, ensure};
use plain_libs::protocols::http_client;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

pub struct Task {
    pub id: String,
    pub file_id: String,
    pub peer: crate::infrastructure::db::DPeer,
    pub total: u64,
    pub generation: String,
}

pub async fn run(
    state: &CoreRuntime,
    task: &Task,
    path: &str,
    attempt: crate::infrastructure::transport::router::Attempt,
) -> Result<()> {
    let peer = crate::features::services::peers::peer_address::current_identity(
        &state.db,
        &task.peer.id,
        &task.peer,
    )?;
    let file_id = task.file_id.as_str();
    let mut available = vec![TransportType::Lan];
    available.extend(
        crate::features::services::peers::peer_transport::capabilities(&state.host)
            .await
            .map_err(plain_libs::anyhow::Error::msg)?
            .into_iter()
            .filter(|kind| *kind != TransportType::Lan),
    );
    let current_peer = crate::features::services::peers::peer_address::current_identity(
        &state.db, &peer.id, &peer,
    )?;
    let step = state
        .transport
        .begin_with_attempt(&current_peer, &available, attempt)?;
    let ticket = step.ticket.ok_or_else(|| {
        plain_libs::anyhow::anyhow!(step.error.unwrap_or_else(|| "No peer transport".into()))
    })?;
    let mut pending = Pending {
        state,
        router: &state.transport,
        ticket,
    };
    let permit = state.lan.capacity.clone().acquire_owned().await?;
    let actor = state
        .prefs
        .get::<String>(crate::features::prefs::pref_keys::CLIENT_ID)?
        .filter(|v| !v.is_empty())
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Missing peer actor"))?;
    let mut file = open_ticket(path).await?;
    let mut bytes = 0;
    loop {
        crate::features::services::peers::peer_transport::notify(state);
        let peer = crate::features::services::peers::peer_address::current_identity(
            &state.db, &peer.id, &peer,
        )?;
        let unavailable = match pending.ticket.transport {
            TransportType::Lan => {
                let url =
                    crate::features::services::peers::peer_address::download_url(&peer, file_id)?;
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
                let url =
                    crate::features::services::peers::peer_address::download_url(&peer, file_id)?;
                let url = http_client::Url::parse(&url)?;
                let path = format!("{}?{}", url.path(), url.query().unwrap_or_default());
                let response = tokio::time::timeout(
                    Duration::from_secs(15),
                    crate::features::services::peers::peer_sdk::aware(
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
                            crate::features::services::peers::peer_lan::incoming_chunks(
                                response.body,
                            );
                        while let Some(chunk) =
                            tokio::time::timeout(Duration::from_secs(30), chunks.next()).await?
                        {
                            write(state, task, &mut file, &mut bytes, &chunk?).await?;
                        }
                        None
                    }
                    Ok(Ok(response)) => Some(format!("Peer HTTP {}", response.status)),
                    Ok(Err(crate::features::services::peers::peer_lan::Failure::Unavailable(
                        error,
                    ))) => Some(error),
                    Ok(Err(crate::features::services::peers::peer_lan::Failure::Fatal(error))) => {
                        return Err(plain_libs::anyhow::anyhow!(error));
                    }
                    Err(error) => Some(error.to_string()),
                }
            }
            TransportType::Ble => {
                let mut offset = 0u64;
                loop {
                    crate::features::services::peers::peer_address::current_identity(
                        &state.db, &peer.id, &peer,
                    )?;
                    let response = crate::features::services::peers::peer_sdk::ble(
                        state,
                        &peer,
                        crate::infrastructure::protocols::ble::Request::FileChunk {
                            client_id: crate::features::services::peers::peer_sdk::ble_actor(state)
                                .map_err(|e| plain_libs::anyhow::anyhow!("{e:?}"))?,
                            file_id: file_id.into(),
                            offset,
                            length: 8192,
                        },
                    )
                    .await
                    .map_err(|e| plain_libs::anyhow::anyhow!("{e:?}"))?;
                    let (status, chunk) =
                        crate::infrastructure::protocols::ble::decode_response(&response)?;
                    if status == 416 {
                        break;
                    }
                    ensure!(
                        status == 206 || (status == 200 && chunk.is_empty()),
                        "BLE file HTTP failure"
                    );
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
                    plain_libs::anyhow::anyhow!(
                        step.error.unwrap_or_else(|| "Peer unavailable".into())
                    )
                })?;
            }
        }
    }
    ensure!(bytes == task.total, "Attachment byte count mismatch");
    crate::features::services::peers::peer_address::current_identity(&state.db, &peer.id, &peer)?;
    file.flush().await?;
    file.sync_all().await?;
    drop(file);
    drop(permit);
    Ok(())
}
async fn write(
    state: &CoreRuntime,
    task: &Task,
    file: &mut tokio::fs::File,
    bytes: &mut u64,
    chunk: &[u8],
) -> Result<()> {
    *bytes = bytes
        .checked_add(chunk.len() as u64)
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Attachment size overflow"))?;
    ensure!(*bytes <= task.total, "Attachment exceeds declared size");
    file.write_all(chunk).await?;
    state
        .downloads
        .progress(&task.id, &task.generation, *bytes)?;
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
#[path = "../../../../../tests/plain-rs/unit/features/services/peers/peer_download.rs"]
mod tests;
