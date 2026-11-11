use crate::{
    managed_peers::{
        PeerRoute, request_error,
        upload::{UploadRequest, UploadResponse},
    },
    server::ServerState,
};
use plain_libs::axum::{body::Bytes, http::StatusCode};
use plain_rs::features::services::files::upload_chunks::MAX_UPLOAD_WRITE_BYTES;
use std::{io, sync::Arc};
use tokio::sync::mpsc;

pub(super) struct PeerUploadChunk {
    state: ServerState,
    route: Arc<PeerRoute>,
    pub(super) sender: Option<mpsc::Sender<io::Result<Bytes>>>,
    pub(super) request: UploadRequest,
    index: i64,
    size: u64,
    written: u64,
    failed: bool,
}

impl PeerUploadChunk {
    pub(super) async fn open(
        state: ServerState,
        peer_id: &str,
        file_id: &str,
        index: i64,
        size: u64,
    ) -> Result<Self, String> {
        plain_rs::features::files::uploads::chunk_path(std::path::Path::new(""), file_id, index)
            .map_err(request_error)?;
        if size > i64::MAX as u64 {
            return Err("Invalid upload chunk size".into());
        }
        let mut stop = state.stop.clone();
        let route = tokio::select! {
            biased;
            _ = stop.wait_for(|value| *value) => return Err("Server stopped".into()),
            route = state.managed_proxy.peers.route(&state, peer_id) => route.map_err(request_error)?,
        };
        let (sender, receiver) = mpsc::channel(2);
        let metadata =
            plain_libs::serde_json::json!({"fileId": file_id, "index": index, "size": size}).to_string();
        let request = crate::managed_peers::upload::request(
            state.clone(),
            route.clone(),
            "upload_chunk",
            &metadata,
            "chunk",
            "application/octet-stream",
            receiver,
        )
        .map_err(request_error)?;
        Ok(Self {
            state,
            route,
            sender: Some(sender),
            request,
            index,
            size,
            written: 0,
            failed: false,
        })
    }

    pub(super) async fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.route
            .check_current(&self.state)
            .map_err(request_error)?;
        if self.failed {
            return Err("upload chunk failed".into());
        }
        if bytes.is_empty() || bytes.len() > MAX_UPLOAD_WRITE_BYTES {
            return Err("invalid upload write length".into());
        }
        let total = self
            .written
            .checked_add(bytes.len() as u64)
            .filter(|total| *total <= self.size)
            .ok_or("upload exceeds declared chunk size")?;
        self.failed = true;
        let sender = self.sender.as_ref().ok_or("upload chunk closed")?;
        tokio::select! {
            biased;
            response = &mut self.request => {
                let response = response.map_err(request_error)?;
                Err(response_error(response).unwrap_or_else(|| "Upload ended before finish".into()))
            }
            permit = sender.reserve() => {
                let permit = permit.map_err(|_| "upload chunk closed")?;
                self.route.check_current(&self.state).map_err(request_error)?;
                permit.send(Ok(Bytes::copy_from_slice(bytes)));
                self.written = total;
                self.failed = false;
                Ok(())
            }
        }
    }

    pub(super) async fn finish(mut self) -> Result<u64, String> {
        if self.failed {
            return Err("upload chunk failed".into());
        }
        if self.written != self.size {
            return Err("upload chunk size mismatch".into());
        }
        self.route
            .check_current(&self.state)
            .map_err(request_error)?;
        self.sender.take();
        let (status, body) = self.request.await.map_err(request_error)?;
        if status != StatusCode::CREATED {
            return Err(response_error((status, body)).unwrap_or_else(|| "Upload failed".into()));
        }
        let text = std::str::from_utf8(&body).map_err(|_| "Invalid upload receipt")?;
        let (index, size) = text.split_once(':').ok_or("Invalid upload receipt")?;
        if index.parse::<i64>().ok() != Some(self.index)
            || size.parse::<u64>().ok() != Some(self.size)
        {
            return Err("Upload receipt mismatch".into());
        }
        Ok(self.written)
    }
}

fn response_error((status, body): UploadResponse) -> Option<String> {
    match status {
        StatusCode::CREATED => None,
        StatusCode::UNAUTHORIZED => Some("unauthorized".into()),
        StatusCode::FORBIDDEN => Some("remote_access_disabled".into()),
        _ => Some(
            String::from_utf8(body)
                .ok()
                .filter(|text| !text.is_empty())
                .unwrap_or_else(|| format!("Upload failed: HTTP {status}")),
        ),
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-server/unit/native/uploads/peer.rs"]
mod tests;
