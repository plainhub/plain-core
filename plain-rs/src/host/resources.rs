use plain_rs::host::bridge::Host;
use plain_rs::host::resource_stream::{NativeResourceFile, NativeResourceStream, ResourceFrame};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot, watch};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceError {
    Busy,
    Unavailable,
    Timeout,
    Closed,
}
impl std::fmt::Display for ResourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Busy => "429 Too Many Requests",
            Self::Unavailable => "503 Service Unavailable",
            Self::Timeout => "504 Gateway Timeout",
            Self::Closed => "502 Bad Gateway",
        })
    }
}
impl std::error::Error for ResourceError {}

struct Pending {
    generation: u64,
    attached: oneshot::Sender<()>,
    outgoing: mpsc::Receiver<ResourceFrame>,
    incoming: mpsc::Sender<ResourceFrame>,
    stop: watch::Receiver<bool>,
}
pub struct NativeResources {
    host: Arc<Host>,
    pending: Mutex<HashMap<String, Pending>>,
    capacity: Arc<Semaphore>,
}
pub struct NativeResourceReader {
    host: Arc<Host>,
    generation: u64,
    connection: watch::Receiver<u64>,
    stop: watch::Receiver<bool>,
    finished: bool,
    _outgoing: mpsc::Sender<ResourceFrame>,
    incoming: mpsc::Receiver<ResourceFrame>,
    _permit: OwnedSemaphorePermit,
}
struct Registration<'a> {
    bridge: &'a NativeResources,
    id: String,
}
impl Drop for Registration<'_> {
    fn drop(&mut self) {
        self.bridge.pending.lock().unwrap().remove(&self.id);
    }
}
impl NativeResources {
    pub fn new(host: Arc<Host>) -> Self {
        Self {
            host,
            pending: Mutex::new(HashMap::new()),
            capacity: Arc::new(Semaphore::new(32)),
        }
    }
    pub fn attach_native(&self, id: &str, generation: u64) -> Result<NativeResourceStream, String> {
        if !self.host.is_current(generation) {
            return Err("host disconnected".into());
        }
        let mut guard = self.pending.lock().unwrap();
        let pending = guard.get(id).ok_or("resource stream unavailable")?;
        if pending.generation != generation || *pending.stop.borrow() {
            return Err("resource stream unavailable".into());
        }
        let pending = guard.remove(id).ok_or("resource stream unavailable")?;
        pending
            .attached
            .send(())
            .map_err(|_| "resource stream cancelled")?;
        Ok(NativeResourceStream {
            host: self.host.clone(),
            generation,
            outgoing: tokio::sync::Mutex::new(pending.outgoing),
            incoming: pending.incoming,
            stop: pending.stop,
            completed: tokio::sync::Mutex::new(false),
        })
    }
    pub async fn read_native(
        &self,
        path: &str,
        stop: watch::Receiver<bool>,
    ) -> Result<NativeResourceReader, ResourceError> {
        self.open(json!({"path":path}), stop).await
    }

    pub async fn read_file(
        &self,
        path: &str,
        stop: watch::Receiver<bool>,
    ) -> Result<NativeResourceFile, String> {
        let mut reader = self
            .open(json!({"path":path,"file":true}), stop)
            .await
            .map_err(|error| error.to_string())?;
        let file = tokio::time::timeout(Duration::from_secs(30), reader.receive())
            .await
            .map_err(|_| "resource file timed out")?;
        match file {
            Some(ResourceFrame::File(file)) => Ok(file),
            _ => Err("resource file unavailable".into()),
        }
    }

    async fn open(
        &self,
        metadata: Value,
        mut stop: watch::Receiver<bool>,
    ) -> Result<NativeResourceReader, ResourceError> {
        let permit = self
            .capacity
            .clone()
            .try_acquire_owned()
            .map_err(|_| ResourceError::Busy)?;
        let mut connection = self.host.connection_changes();
        let generation = self.host.generation().ok_or(ResourceError::Unavailable)?;
        let id = uuid::Uuid::new_v4().to_string();
        let (attached_tx, attached_rx) = oneshot::channel();
        let (outgoing, receiver) = mpsc::channel(4);
        let (sender, incoming) = mpsc::channel(4);
        self.pending.lock().unwrap().insert(
            id.clone(),
            Pending {
                generation,
                attached: attached_tx,
                outgoing: receiver,
                incoming: sender,
                stop: stop.clone(),
            },
        );
        let _registration = Registration {
            bridge: self,
            id: id.clone(),
        };
        tokio::select! {
            _ = stop.changed() => return Err(ResourceError::Unavailable),
            _ = connection.changed() => return Err(ResourceError::Unavailable),
            result = self.host.call("fileResourceStream", json!({"id":id})) => {
                result.map_err(|_| ResourceError::Unavailable)?;
            }
        }
        tokio::select! {
            _ = stop.changed() => return Err(ResourceError::Unavailable),
            _ = connection.changed() => return Err(ResourceError::Unavailable),
            result = tokio::time::timeout(Duration::from_secs(15), attached_rx) => {
                result.map_err(|_| ResourceError::Timeout)?.map_err(|_| ResourceError::Closed)?;
            }
        }
        let send = outgoing.send(ResourceFrame::Request(metadata.to_string()));
        tokio::select! {
            _ = stop.changed() => return Err(ResourceError::Unavailable),
            _ = connection.changed() => return Err(ResourceError::Unavailable),
            result = tokio::time::timeout(Duration::from_secs(15), send) => {
                result.map_err(|_| ResourceError::Timeout)?.map_err(|_| ResourceError::Closed)?;
            }
        }
        Ok(NativeResourceReader {
            host: self.host.clone(),
            generation,
            connection,
            stop,
            finished: false,
            _outgoing: outgoing,
            incoming,
            _permit: permit,
        })
    }
}

impl NativeResourceReader {
    async fn receive(&mut self) -> Option<ResourceFrame> {
        if *self.stop.borrow() || !self.host.is_current(self.generation) {
            return None;
        }
        tokio::select! {
            _ = self.stop.changed() => None,
            _ = self.connection.changed() => None,
            message = self.incoming.recv() => message,
        }
    }
    pub async fn read(&mut self) -> Result<Option<Vec<u8>>, String> {
        if self.finished {
            return Ok(None);
        }
        let message = self.receive().await;
        match message {
            Some(ResourceFrame::Bytes(bytes)) => Ok(Some(bytes)),
            Some(ResourceFrame::End) => {
                self.finished = true;
                Ok(None)
            }
            _ => {
                self.finished = true;
                Err("resource stream interrupted".into())
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/host/resources.rs"]
mod tests;
