use super::{GRAPHQL_LIMIT, PeerRoute};
use crate::server::ServerState;
use anyhow::{Result, ensure};
use axum::{
    body::Bytes,
    http::{StatusCode, header},
};
use futures_util::StreamExt;
use plain_libs::protocols::http_client;
use plain_rs::infrastructure::transport::http_request;
use std::{future::Future, io, pin::Pin, sync::Arc};
use tokio::{
    sync::{mpsc, watch},
    time::{Duration, Instant},
};

const UPLOAD_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) type UploadResponse = (StatusCode, Vec<u8>);
pub(crate) type UploadRequest = Pin<Box<dyn Future<Output = Result<UploadResponse>> + Send>>;

pub(crate) fn request(
    state: ServerState,
    route: Arc<PeerRoute>,
    path: &str,
    metadata: &str,
    filename: &str,
    mime: &str,
    rx: mpsc::Receiver<io::Result<Bytes>>,
) -> Result<UploadRequest> {
    ensure!(
        matches!(path, "upload" | "upload_chunk"),
        "Invalid upload path"
    );
    ensure!(metadata.len() <= 64 * 1024, "Upload metadata too large");
    ensure!(
        route.supports_managed_resources,
        "Managed upload unavailable"
    );
    route.check_current(&state)?;
    let filename = filename.rsplit(['/', '\\']).next().unwrap_or_default();
    ensure!(
        !filename.is_empty() && !filename.contains(['\r', '\n', '\0']),
        "Invalid upload filename"
    );
    ensure!(
        !mime.contains(['\r', '\n', '\0']),
        "Invalid upload content type"
    );
    let filename = filename.replace('"', "\\\"");
    let boundary = plain_libs::utils::uuid::Uuid::new_v4().to_string();
    let encrypted = http_request::encrypt(&route.key, &format!("/{path}"), metadata)
        .map_err(anyhow::Error::msg)?;
    let mut prefix =
        format!("--{boundary}\r\nContent-Disposition: form-data; name=\"info\"\r\n\r\n")
            .into_bytes();
    prefix.extend_from_slice(&encrypted);
    prefix.extend_from_slice(format!("\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: {mime}\r\n\r\n").as_bytes());
    let suffix = Bytes::from(format!("\r\n--{boundary}--\r\n"));
    let data = futures_util::stream::unfold(rx, |mut rx| async {
        rx.recv().await.map(|chunk| (chunk, rx))
    });
    let (progress, updates) = watch::channel(Instant::now());
    let keep_progress = progress.clone();
    let stream = futures_util::stream::once(async { Ok::<_, io::Error>(Bytes::from(prefix)) })
        .chain(data)
        .chain(futures_util::stream::once(async { Ok(suffix) }))
        .inspect(move |_| {
            progress.send_replace(Instant::now());
        });
    let outgoing = route
        .client
        .post(route.base.join(&format!("/{path}"))?)
        .read_timeout(None)
        .header("c-id", &route.client_id)
        .header("x-plain-managed-resource", "1")
        .header(
            header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(http_client::Body::wrap_stream(stream));
    Ok(Box::pin(async move {
        let _progress = keep_progress;
        let mut stop = state.stop.clone();
        tokio::select! {
            biased;
            _ = stop.wait_for(|value| *value) => anyhow::bail!("Server stopped"),
            _ = wait_for_stall(updates) => anyhow::bail!("connection_timeout"),
            response = async {
                route.check_current(&state)?;
                let response = outgoing.send().await?;
                let status = response.status();
                ensure!(!status.is_redirection(), "Upload redirect refused");
                let body = plain_rs::features::services::peers::peer_lan::limited(response, GRAPHQL_LIMIT).await?;
                route.check_current(&state)?;
                Ok((status, body))
            } => response,
        }
    }))
}

async fn wait_for_stall(mut progress: watch::Receiver<Instant>) {
    loop {
        let deadline = *progress.borrow_and_update() + UPLOAD_IDLE_TIMEOUT;
        tokio::select! {
            biased;
            changed = progress.changed() => if changed.is_err() { return; },
            _ = tokio::time::sleep_until(deadline) => return,
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-server/unit/managed_peers/upload.rs"]
mod tests;
