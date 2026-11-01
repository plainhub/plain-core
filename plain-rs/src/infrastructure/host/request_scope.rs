use tokio::sync::watch;
#[derive(Clone)]
pub struct NativeRequestScope {
    id: String,
    closed: watch::Sender<bool>,
}

impl Default for NativeRequestScope {
    fn default() -> Self {
        let (closed, _) = watch::channel(false);
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            closed,
        }
    }
}

impl NativeRequestScope {
    pub fn cancellation(&self) -> watch::Receiver<bool> {
        self.closed.subscribe()
    }
    pub async fn wait_closed(&self) {
        let mut closed = self.closed.subscribe();
        let _ = closed.wait_for(|closed| *closed).await;
    }

    pub fn is_closed(&self) -> bool {
        *self.closed.borrow()
    }

    pub fn close(&self) {
        self.closed.send_replace(true);
    }
    pub fn id(&self) -> &str {
        &self.id
    }
}
