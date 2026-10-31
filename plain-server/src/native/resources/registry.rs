use super::{
    MAX_RESOURCE_READ_BYTES, MAX_RESOURCE_RESPONSE_BYTES, NativeFileResources, ResourceInfo,
};
use plain_rs::host::request_scope::NativeRequestScope;
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
        let read = async {
            let mut bytes = Vec::with_capacity(length as usize);
            while (bytes.len() as u64) < length {
                let count =
                    (length - bytes.len() as u64).min(MAX_RESOURCE_READ_BYTES as u64) as usize;
                let chunk = self
                    .store
                    .read(&self.info.id, offset + bytes.len() as u64, count)
                    .await?;
                if chunk.is_empty() {
                    return Err("resource ended before the requested range".into());
                }
                bytes.extend_from_slice(&chunk);
            }
            Ok(bytes)
        };
        tokio::select! {
            biased;
            _ = self.scope.wait_closed() => Err("resource owner closed".into()),
            result = read => result,
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

#[derive(Default)]
pub struct NativeResourceRegistry {
    state: Mutex<State>,
}

impl NativeResourceRegistry {
    pub async fn open(
        &self,
        scope: &NativeRequestScope,
        path: &Path,
    ) -> Result<ResourceInfo, String> {
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
            result = store.open(path) => result?,
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
        if scope.is_closed() {
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
#[path = "../../../tests/unit/native/resources/registry.rs"]
mod tests;
