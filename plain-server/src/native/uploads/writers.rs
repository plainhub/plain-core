use super::NativeUploads;
use plain_rs::host::request_scope::NativeRequestScope;
use plain_rs::services::files::upload_chunks::UploadChunk;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::watch;

struct Slot {
    owner: String,
    writer: tokio::sync::Mutex<Option<UploadChunk>>,
    closed: AtomicBool,
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
            result = uploads.open_chunk(file_id, index, size) => result?,
        };
        let mut registry = self.registry.lock().unwrap();
        ensure_open(scope)?;
        if canceled.has_changed().is_err() {
            return Err("upload chunk canceled".into());
        }
        let handle = uuid::Uuid::new_v4().to_string();
        registry.slots.insert(
            handle.clone(),
            Arc::new(Slot {
                owner: scope.id().to_owned(),
                writer: tokio::sync::Mutex::new(Some(writer)),
                closed: AtomicBool::new(false),
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
        let mut writer = slot.writer.lock().await;
        if slot.closed.load(Ordering::SeqCst) {
            return Err("upload chunk canceled".into());
        }
        let chunk = writer.as_mut().ok_or("upload chunk unavailable")?;
        tokio::select! {
            biased;
            _ = scope.wait_closed() => Err("Native request scope closed".into()),
            result = chunk.write(bytes) => result,
        }
    }

    pub async fn finish(&self, scope: &NativeRequestScope, handle: &str) -> Result<u64, String> {
        let slot = self.owned(scope, handle)?;
        let mut writer = slot.writer.lock().await;
        let chunk = {
            let mut registry = self.registry.lock().unwrap();
            ensure_open(scope)?;
            if slot.closed.load(Ordering::SeqCst) {
                return Err("upload chunk canceled".into());
            }
            let chunk = writer.take().ok_or("upload chunk unavailable")?;
            slot.closed.store(true, Ordering::SeqCst);
            registry.slots.remove(handle);
            chunk
        };
        tokio::select! {
            biased;
            _ = scope.wait_closed() => Err("Native request scope closed".into()),
            result = chunk.finish() => result,
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
                slot.closed.store(true, Ordering::SeqCst);
            }
        }
    }

    pub fn abort_owner(&self, owner: &str) {
        let mut registry = self.registry.lock().unwrap();
        registry.owners.remove(owner);
        registry.slots.retain(|_, slot| {
            if slot.owner == owner {
                slot.closed.store(true, Ordering::SeqCst);
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
#[path = "../../../tests/unit/native/uploads/writers.rs"]
mod tests;
