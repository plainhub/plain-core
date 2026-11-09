use super::{NativeUploads, peer::PeerUploadChunk};
use plain_rs::features::services::files::upload_chunks::UploadChunk;
use plain_rs::infrastructure::host::request_scope::NativeRequestScope;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::watch;

struct Slot {
    owner: String,
    writer: tokio::sync::Mutex<Option<Writer>>,
    closed: watch::Sender<bool>,
}

enum Writer {
    Local(UploadChunk),
    Peer(PeerUploadChunk),
}

impl Writer {
    async fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        match self {
            Self::Local(writer) => writer.write(bytes).await,
            Self::Peer(writer) => writer.write(bytes).await,
        }
    }
    async fn finish(self) -> Result<u64, String> {
        match self {
            Self::Local(writer) => writer.finish().await,
            Self::Peer(writer) => writer.finish().await,
        }
    }
}

struct Operation<'a> {
    writers: &'a UploadWriters,
    scope: &'a NativeRequestScope,
    handle: &'a str,
    keep: bool,
}

impl Drop for Operation<'_> {
    fn drop(&mut self) {
        if !self.keep {
            self.writers.abort(self.scope, self.handle);
        }
    }
}

#[derive(Default)]
struct Registry {
    slots: HashMap<String, Arc<Slot>>,
    owners: HashMap<String, watch::Sender<()>>,
}

#[derive(Default)]
pub struct UploadWriters {
    registry: Mutex<Registry>,
}

impl UploadWriters {
    pub async fn open(
        &self,
        uploads: &NativeUploads,
        scope: &NativeRequestScope,
        file_id: &str,
        index: i64,
        size: u64,
    ) -> Result<String, String> {
        self.open_with(scope, async {
            uploads
                .open_chunk(file_id, index, size)
                .await
                .map(Writer::Local)
        })
        .await
    }

    pub async fn open_peer(
        &self,
        uploads: &NativeUploads,
        scope: &NativeRequestScope,
        peer_id: &str,
        file_id: &str,
        index: i64,
        size: u64,
    ) -> Result<String, String> {
        self.open_with(scope, async {
            PeerUploadChunk::open(uploads.state.clone(), peer_id, file_id, index, size)
                .await
                .map(Writer::Peer)
        })
        .await
    }

    async fn open_with(
        &self,
        scope: &NativeRequestScope,
        opening: impl std::future::Future<Output = Result<Writer, String>>,
    ) -> Result<String, String> {
        ensure_open(scope)?;
        let mut canceled = {
            let mut registry = self.registry.lock().unwrap();
            ensure_open(scope)?;
            registry
                .owners
                .entry(scope.id().to_owned())
                .or_insert_with(|| watch::channel(()).0)
                .subscribe()
        };
        let writer = tokio::select! {
            biased;
            _ = scope.wait_closed() => return Err("Native request scope closed".into()),
            _ = canceled.changed() => return Err("upload chunk canceled".into()),
            result = opening => result?,
        };
        let mut registry = self.registry.lock().unwrap();
        ensure_open(scope)?;
        if canceled.has_changed().is_err() {
            return Err("upload chunk canceled".into());
        }
        let handle = plain_libs::utils::uuid::Uuid::new_v4().into_string();
        registry.slots.insert(
            handle.clone(),
            Arc::new(Slot {
                owner: scope.id().to_owned(),
                writer: tokio::sync::Mutex::new(Some(writer)),
                closed: watch::channel(false).0,
            }),
        );
        Ok(handle)
    }

    fn owned(&self, scope: &NativeRequestScope, handle: &str) -> Result<Arc<Slot>, String> {
        ensure_open(scope)?;
        self.registry
            .lock()
            .unwrap()
            .slots
            .get(handle)
            .filter(|slot| slot.owner == scope.id())
            .cloned()
            .ok_or_else(|| "upload chunk unavailable".into())
    }

    pub async fn write(
        &self,
        scope: &NativeRequestScope,
        handle: &str,
        bytes: &[u8],
    ) -> Result<(), String> {
        let slot = self.owned(scope, handle)?;
        let mut operation = Operation {
            writers: self,
            scope,
            handle,
            keep: false,
        };
        let mut canceled = slot.closed.subscribe();
        let result = tokio::select! {
            biased;
            _ = scope.wait_closed() => Err("Native request scope closed".into()),
            _ = canceled.wait_for(|closed| *closed) => Err("upload chunk canceled".into()),
            result = async {
                let mut writer = slot.writer.lock().await;
                if *slot.closed.borrow() { return Err("upload chunk canceled".into()); }
                writer.as_mut().ok_or("upload chunk unavailable")?.write(bytes).await
            } => result,
        };
        operation.keep = result.is_ok();
        result
    }

    pub async fn finish(&self, scope: &NativeRequestScope, handle: &str) -> Result<u64, String> {
        let slot = self.owned(scope, handle)?;
        let _operation = Operation {
            writers: self,
            scope,
            handle,
            keep: false,
        };
        let mut canceled = slot.closed.subscribe();
        tokio::select! {
            biased;
            _ = scope.wait_closed() => Err("Native request scope closed".into()),
            _ = canceled.wait_for(|closed| *closed) => Err("upload chunk canceled".into()),
            result = async {
                let mut writer = slot.writer.lock().await;
                ensure_open(scope)?;
                if *slot.closed.borrow() { return Err("upload chunk canceled".into()); }
                writer.take().ok_or("upload chunk unavailable")?.finish().await
            } => result,
        }
    }

    pub fn abort(&self, scope: &NativeRequestScope, handle: &str) {
        let mut registry = self.registry.lock().unwrap();
        if registry
            .slots
            .get(handle)
            .is_some_and(|slot| slot.owner == scope.id())
        {
            if let Some(slot) = registry.slots.remove(handle) {
                slot.closed.send_replace(true);
            }
        }
    }

    pub fn abort_owner(&self, owner: &str) {
        let mut registry = self.registry.lock().unwrap();
        registry.owners.remove(owner);
        registry.slots.retain(|_, slot| {
            if slot.owner == owner {
                slot.closed.send_replace(true);
                false
            } else {
                true
            }
        });
    }
}

fn ensure_open(scope: &NativeRequestScope) -> Result<(), String> {
    if scope.is_closed() {
        return Err("Native request scope closed".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../../tests/plain-server/unit/native/uploads/writers.rs"]
mod tests;
