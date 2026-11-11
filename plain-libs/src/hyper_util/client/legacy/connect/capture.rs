use super::Connected;
use crate::http::Request;
use std::{ops::Deref, sync::Arc};
use tokio::sync::watch;
#[derive(Debug, Clone)]
pub struct CaptureConnection {
    rx: watch::Receiver<Option<Connected>>,
}
pub fn capture_connection<B>(request: &mut Request<B>) -> CaptureConnection {
    let (tx, rx) = CaptureConnection::new();
    request.extensions_mut().insert(tx);
    rx
}
#[derive(Clone)]
pub(crate) struct CaptureConnectionExtension {
    tx: Arc<watch::Sender<Option<Connected>>>,
}
impl CaptureConnectionExtension {
    pub(crate) fn set(&self, connected: &Connected) {
        self.tx.send_replace(Some(connected.clone()));
    }
}
impl CaptureConnection {
    pub(crate) fn new() -> (CaptureConnectionExtension, Self) {
        let (tx, rx) = watch::channel(None);
        (
            CaptureConnectionExtension { tx: Arc::new(tx) },
            CaptureConnection { rx },
        )
    }
}
