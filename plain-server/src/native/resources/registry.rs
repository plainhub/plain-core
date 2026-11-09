use super::{MAX_RESOURCE_RESPONSE_BYTES, NativeFileResources, ResourceInfo};
use plain_rs::infrastructure::host::request_scope::NativeRequestScope;
use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct NativeResource {
    pub info: ResourceInfo,
    scope: NativeRequestScope,
    store: Arc<NativeFileResources>,
    reads: Arc<tokio::sync::Semaphore>,
}

impl NativeResource {
    pub async fn read_range(&self, offset: u64, length: u64) -> Result<Vec<u8>, String> {
        if length > MAX_RESOURCE_RESPONSE_BYTES {
            return Err("resource response exceeds limit".into());
        }
        if offset
            .checked_add(length)
            .is_none_or(|end| end > self.info.size)
        {
            return Err("resource range exceeds size".into());
        }
        ensure_open(&self.scope)?;
        if !self.store.is_open(&self.info.id) {
            return Err("resource unavailable".into());
        }
        tokio::select! {
            biased;
            _ = self.scope.wait_closed() => Err("resource owner closed".into()),
            result = async {
                let _permit = self.reads.acquire().await.map_err(|_| "resource registry closed")?;
                self.store.read_range(&self.info.id, offset, length).await
            } => result,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ResourceAccessError {
    Unavailable,
    Forbidden,
}

#[derive(Default)]
struct State {
    stores: HashMap<String, Arc<NativeFileResources>>,
    handles: HashMap<String, NativeResource>,
}

pub struct NativeResourceRegistry {
    state: Mutex<State>,
    reads: Arc<tokio::sync::Semaphore>,
    prepares: tokio::sync::Semaphore,
}

impl Default for NativeResourceRegistry {
    fn default() -> Self {
        Self {
            state: Mutex::default(),
            reads: Arc::new(tokio::sync::Semaphore::new(4)),
            prepares: tokio::sync::Semaphore::new(4),
        }
    }
}

impl NativeResourceRegistry {
    pub async fn open(
        &self,
        scope: &NativeRequestScope,
        path: &Path,
    ) -> Result<ResourceInfo, String> {
        self.open_with(scope, |store| async move { store.open(path).await })
            .await
    }

    pub(super) async fn open_content(
        &self,
        scope: &NativeRequestScope,
        content: plain_rs::features::services::files::content::Content,
        stop: Option<tokio::sync::watch::Receiver<bool>>,
    ) -> Result<ResourceInfo, String> {
        self.open_with(scope, |store| async move {
            store.open_content(content, stop).await
        })
        .await
    }

    pub(super) async fn prepare(&self) -> Result<tokio::sync::SemaphorePermit<'_>, String> {
        self.prepares
            .acquire()
            .await
            .map_err(|_| "resource registry closed".into())
    }

    pub(super) async fn open_staged(
        &self,
        scope: &NativeRequestScope,
        file: plain_libs::temporary::NamedTempFile,
        name: String,
        mime: String,
        stop: Option<tokio::sync::watch::Receiver<bool>>,
    ) -> Result<ResourceInfo, String> {
        self.open_with(scope, |store| async move {
            store.open_staged(file, name, mime, stop).await
        })
        .await
    }

    pub(super) async fn open_peer(
        &self,
        scope: &NativeRequestScope,
        reference: &str,
        peer: crate::managed_peers::resource::PeerResource,
    ) -> Result<ResourceInfo, String> {
        self.open_with(scope, |store| async move {
            store.insert(
                reference.rsplit('/').next().unwrap_or(reference).into(),
                peer.mime.clone(),
                peer.size,
                super::Source::Peer(peer),
                None,
            )
        })
        .await
    }

    async fn open_with<F, Fut>(
        &self,
        scope: &NativeRequestScope,
        open: F,
    ) -> Result<ResourceInfo, String>
    where
        F: FnOnce(Arc<NativeFileResources>) -> Fut,
        Fut: std::future::Future<Output = Result<ResourceInfo, String>>,
    {
        let store = {
            let mut state = self.state.lock().unwrap();
            ensure_open(scope)?;
            state
                .stores
                .entry(scope.id().to_owned())
                .or_default()
                .clone()
        };
        let info = tokio::select! {
            biased;
            _ = scope.wait_closed() => return Err("resource owner closed".into()),
            result = open(store.clone()) => result?,
        };
        let mut state = self.state.lock().unwrap();
        if scope.is_closed()
            || !state
                .stores
                .get(scope.id())
                .is_some_and(|current| Arc::ptr_eq(current, &store))
        {
            store.close(&info.id);
            return Err("resource owner closed".into());
        }
        state.handles.insert(
            info.id.clone(),
            NativeResource {
                info: info.clone(),
                scope: scope.clone(),
                store,
                reads: self.reads.clone(),
            },
        );
        Ok(info)
    }

    pub fn get(
        &self,
        scope: &NativeRequestScope,
        id: &str,
    ) -> Result<NativeResource, ResourceAccessError> {
        let state = self.state.lock().unwrap();
        let resource = state
            .handles
            .get(id)
            .ok_or(ResourceAccessError::Unavailable)?;
        if resource.scope.id() != scope.id() {
            return Err(ResourceAccessError::Forbidden);
        }
        if scope.is_closed() || !resource.store.is_open(id) {
            return Err(ResourceAccessError::Unavailable);
        }
        Ok(resource.clone())
    }

    pub fn close(&self, scope: &NativeRequestScope, id: &str) -> bool {
        let mut state = self.state.lock().unwrap();
        if !state
            .handles
            .get(id)
            .is_some_and(|resource| resource.scope.id() == scope.id())
        {
            return false;
        }
        if let Some(resource) = state.handles.remove(id) {
            resource.store.close(id);
        }
        true
    }

    pub fn close_owner(&self, owner: &str) {
        let mut state = self.state.lock().unwrap();
        state
            .handles
            .retain(|_, resource| resource.scope.id() != owner);
        if let Some(store) = state.stores.remove(owner) {
            store.close_all();
        }
    }
}

fn ensure_open(scope: &NativeRequestScope) -> Result<(), String> {
    if scope.is_closed() {
        return Err("resource owner closed".into());
    }
    Ok(())
}

impl Drop for NativeResourceRegistry {
    fn drop(&mut self) {
        for store in self.state.get_mut().unwrap().stores.values() {
            store.close_all();
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-server/unit/native/resources/registry.rs"]
mod tests;
