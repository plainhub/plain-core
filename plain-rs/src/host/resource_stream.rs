use plain_rs::host::bridge::Host;
use std::sync::Arc;
use tokio::sync::{mpsc, watch};

pub enum ResourceFrame {
    Request(String),
    Bytes(Vec<u8>),
    File(NativeResourceFile),
    End,
    Error,
}

pub struct NativeResourceFile {
    pub file: std::fs::File,
    pub offset: i64,
    pub length: i64,
}

impl NativeResourceFile {
    pub fn into_reader(
        self,
    ) -> Result<plain_rs::utils::range_reader::RangeReader<std::fs::File>, String> {
        let base = u64::try_from(self.offset).map_err(|error| error.to_string())?;
        let length = u64::try_from(self.length).map_err(|error| error.to_string())?;
        plain_rs::utils::range_reader::RangeReader::new(self.file, base, length)
            .map_err(|error| error.to_string())
    }
    pub fn new(file: std::fs::File, offset: i64, length: i64) -> Result<Self, String> {
        if offset < 0 || length < -1 {
            return Err("invalid resource file range".into());
        }
        let metadata = file.metadata().map_err(|error| error.to_string())?;
        let length = if length == -1 && metadata.is_file() && metadata.len() > 0 {
            let size = i64::try_from(metadata.len()).map_err(|error| error.to_string())?;
            size.checked_sub(offset)
                .filter(|length| *length >= 0)
                .ok_or("invalid resource file range")?
        } else {
            length
        };
        if length >= 0 {
            offset
                .checked_add(length)
                .ok_or("invalid resource file range")?;
        }
        Ok(Self {
            file,
            offset,
            length,
        })
    }
}

pub struct NativeResourceStream {
    pub host: Arc<Host>,
    pub generation: u64,
    pub outgoing: tokio::sync::Mutex<mpsc::Receiver<ResourceFrame>>,
    pub incoming: mpsc::Sender<ResourceFrame>,
    pub stop: watch::Receiver<bool>,
    pub completed: tokio::sync::Mutex<bool>,
}

impl NativeResourceStream {
    pub async fn next_request(&self) -> Result<Option<String>, String> {
        let mut connection = self.host.connection_changes();
        let mut stop = self.stop.clone();
        if *stop.borrow() || !self.host.is_current(self.generation) {
            return Err("resource stream closed".into());
        }
        let mut outgoing = tokio::select! {
            _ = stop.changed() => return Err("resource stream closed".into()),
            _ = connection.changed() => return Err("host disconnected".into()),
            outgoing = self.outgoing.lock() => outgoing,
        };
        tokio::select! {
            _ = stop.changed() => Err("resource stream closed".into()),
            _ = connection.changed() => Err("host disconnected".into()),
            value = outgoing.recv() => match value {
                Some(ResourceFrame::Request(value)) => Ok(Some(value)),
                None => Ok(None),
                _ => Err("invalid resource request".into()),
            },
        }
    }

    pub async fn write(&self, bytes: Vec<u8>) -> Result<(), String> {
        if bytes.is_empty() || bytes.len() > 64 * 1024 {
            return Err("invalid resource chunk size".into());
        }
        self.send(ResourceFrame::Bytes(bytes), false).await
    }

    pub async fn file(&self, file: NativeResourceFile) -> Result<(), String> {
        if file.offset < 0
            || file.length < -1
            || (file.length >= 0 && file.offset.checked_add(file.length).is_none())
        {
            return Err("invalid resource file range".into());
        }
        self.send(ResourceFrame::File(file), true).await
    }

    pub async fn finish(&self) -> Result<(), String> {
        self.send(ResourceFrame::End, true).await
    }

    pub async fn fail(&self) -> Result<(), String> {
        self.send(ResourceFrame::Error, true).await
    }

    async fn send(&self, message: ResourceFrame, terminal: bool) -> Result<(), String> {
        let mut connection = self.host.connection_changes();
        let mut stop = self.stop.clone();
        if *stop.borrow() || !self.host.is_current(self.generation) {
            return Err("resource stream closed".into());
        }
        let mut completed = tokio::select! {
            _ = stop.changed() => return Err("resource stream closed".into()),
            _ = connection.changed() => return Err("host disconnected".into()),
            completed = self.completed.lock() => completed,
        };
        if *completed {
            return Err("resource stream finished".into());
        }
        let result = tokio::select! {
            _ = stop.changed() => Err("resource stream closed".into()),
            _ = connection.changed() => Err("host disconnected".into()),
            result = self.incoming.send(message) => result.map_err(|_| "resource stream consumer closed".into()),
        };
        if result.is_ok() && terminal {
            *completed = true;
        }
        result
    }
}
