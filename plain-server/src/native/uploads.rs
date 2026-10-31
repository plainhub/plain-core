use plain_rs::services::files::upload_chunks::{UploadChunk, UploadChunks};
use std::sync::Arc;
use tokio::sync::watch;

mod writers;
pub use writers::UploadWriters;

pub const MAX_NATIVE_UPLOAD_CHUNK_BYTES: usize = 1024 * 1024;

#[derive(Clone)]
pub struct NativeUploads {
    chunks: Arc<UploadChunks>,
    stop: watch::Receiver<bool>,
}

impl NativeUploads {
    pub(crate) fn new(chunks: Arc<UploadChunks>, stop: watch::Receiver<bool>) -> Self {
        Self { chunks, stop }
    }

    pub async fn write_chunk(&self, id: &str, index: i64, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > MAX_NATIVE_UPLOAD_CHUNK_BYTES {
            return Err("Invalid upload chunk length".into());
        }
        let mut chunk = self.open_chunk(id, index, bytes.len() as u64).await?;
        for bytes in bytes.chunks(plain_rs::services::files::upload_chunks::MAX_UPLOAD_WRITE_BYTES)
        {
            chunk.write(bytes).await?;
        }
        chunk.finish().await?;
        Ok(())
    }

    pub async fn open_chunk(&self, id: &str, index: i64, size: u64) -> Result<UploadChunk, String> {
        ensure_running(&self.stop)?;
        self.chunks
            .open_chunk(id, index, size, self.stop.clone())
            .await
    }
}

fn ensure_running(stop: &watch::Receiver<bool>) -> Result<(), String> {
    if *stop.borrow() || stop.has_changed().is_err() {
        return Err("Rust core stopped".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/native/uploads.rs"]
mod tests;
