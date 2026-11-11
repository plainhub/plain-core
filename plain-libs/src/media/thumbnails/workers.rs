use std::sync::{Arc, Mutex, mpsc};

type Job = Box<dyn FnOnce() + Send>;

pub(super) struct Workers {
    sender: mpsc::Sender<Job>,
}

impl Workers {
    pub(super) fn new(count: usize) -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::channel::<Job>();
        let receiver = Arc::new(Mutex::new(receiver));
        for index in 0..count {
            let receiver = receiver.clone();
            std::thread::Builder::new()
                .name(format!("thumbnail-{index}"))
                .spawn(move || {
                    loop {
                        let job = receiver.lock().unwrap().recv();
                        match job {
                            Ok(job) => job(),
                            Err(_) => break,
                        }
                    }
                })?;
        }
        Ok(Self { sender })
    }

    pub(super) async fn run<T: Send + 'static>(
        &self,
        job: impl FnOnce() -> T + Send + 'static,
    ) -> crate::anyhow::Result<T> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.sender
            .send(Box::new(move || {
                if sender.is_closed() {
                    return;
                }
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(job)).map_err(|panic| {
                        let message = panic
                            .downcast_ref::<String>()
                            .map(String::as_str)
                            .or_else(|| panic.downcast_ref::<&str>().copied())
                            .unwrap_or("unknown panic payload");
                        crate::anyhow::anyhow!("thumbnail worker panicked: {message}")
                    });
                let _ = sender.send(result);
            }))
            .map_err(|_| crate::anyhow::anyhow!("thumbnail workers stopped"))?;
        receiver.await?
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/media/thumbnails/workers.rs"]
mod tests;
