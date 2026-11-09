use crate::server::ServerState;
use plain_rs::features::services::files::upload_chunks::UploadChunk;
use tokio::sync::watch;

pub use plain_rs::features::services::files::upload_chunks::MAX_UPLOAD_WRITE_BYTES;

mod peer;
mod writers;
pub use writers::UploadWriters;

pub const MAX_NATIVE_UPLOAD_CHUNK_BYTES: usize = 1024 * 1024;

#[derive(Clone)]
pub struct NativeUploads {
    state: ServerState,
}

impl NativeUploads {
    pub(crate) fn new(state: ServerState) -> Self {
        Self { state }
    }

    pub async fn write_chunk(&self, id: &str, index: i64, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > MAX_NATIVE_UPLOAD_CHUNK_BYTES {
            return Err("Invalid upload chunk length".into());
        }
        let mut chunk = self.open_chunk(id, index, bytes.len() as u64).await?;
        for bytes in
            bytes.chunks(plain_rs::features::services::files::upload_chunks::MAX_UPLOAD_WRITE_BYTES)
        {
            chunk.write(bytes).await?;
        }
        chunk.finish().await?;
        Ok(())
    }

    pub async fn open_chunk(&self, id: &str, index: i64, size: u64) -> Result<UploadChunk, String> {
        ensure_running(&self.state.stop)?;
        self.state
            .upload_chunks
            .open_chunk(id, index, size, self.state.stop.clone())
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
#[path = "../../../tests/plain-server/unit/native/uploads.rs"]
mod tests;

#[cfg(all(test, feature = "http_transport"))]
#[path = "../../../tests/plain-server/unit/native/uploads/fixture.rs"]
mod fixture;

#[cfg(test)]
#[path = "../../../tests/plain-server/unit/native/uploads/local.rs"]
mod local_fixture;
