use super::{CoreRuntime, Request, Thumbnails};
use futures_util::{StreamExt, stream::FuturesUnordered};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::sync::{mpsc, watch};

type Output = Result<Option<Vec<u8>>, String>;
type Reply = Option<Arc<Output>>;

pub(crate) struct Generation {
    pub(super) request: Request,
    pub(super) cache: PathBuf,
    pub(super) key: String,
    pub(super) cached: PathBuf,
}

pub(in crate::features::services::images) struct Queue {
    sender: mpsc::Sender<Generation>,
    replies: Mutex<HashMap<String, watch::Sender<Reply>>>,
}

impl Queue {
    pub(super) fn new() -> (Self, mpsc::Receiver<Generation>) {
        let (sender, receiver) = mpsc::channel(320);
        (
            Self {
                sender,
                replies: Mutex::new(HashMap::new()),
            },
            receiver,
        )
    }

    pub(super) async fn submit(
        &self,
        request: Request,
        cache: PathBuf,
        key: String,
        cached: PathBuf,
    ) -> Output {
        let permit = self
            .sender
            .reserve()
            .await
            .map_err(|_| "Rust core stopped")?;
        let mut reply = {
            let mut replies = self.replies.lock().unwrap();
            if let Some(reply) = replies.get(&key) {
                drop(permit);
                reply.subscribe()
            } else {
                let (tx, rx) = watch::channel(None);
                replies.insert(key.clone(), tx);
                permit.send(Generation {
                    request,
                    cache,
                    key,
                    cached,
                });
                rx
            }
        };
        reply.changed().await.map_err(|_| "Rust core stopped")?;
        let result = reply.borrow_and_update().as_ref().unwrap().clone();
        drop(reply);
        Arc::unwrap_or_clone(result)
    }

    pub(super) async fn run(state: CoreRuntime, mut receiver: mpsc::Receiver<Generation>) {
        const WORKERS: usize = 4;
        let mut stop = state.stop.clone();
        let mut running = FuturesUnordered::new();
        let mut batch = Vec::with_capacity(WORKERS);
        loop {
            let slots = WORKERS - running.len();
            tokio::select! {
                _ = stop.wait_for(|value| *value) => break,
                _ = running.next(), if !running.is_empty() => {},
                count = receiver.recv_many(&mut batch, slots), if slots > 0 => {
                    if count == 0 { break; }
                    for job in batch.drain(..) {
                        let mut replies = state.thumbnails.queue.replies.lock().unwrap();
                        if replies[&job.key].receiver_count() == 0 {
                            replies.remove(&job.key);
                        } else {
                            running.push(Self::complete(&state, job));
                        }
                    }
                }
            }
        }
        drop(receiver);
        while running.next().await.is_some() {}
        state.thumbnails.queue.replies.lock().unwrap().clear();
    }

    async fn complete(state: &CoreRuntime, job: Generation) {
        let result = Thumbnails::generate(state, &job).await;
        let reply = state
            .thumbnails
            .queue
            .replies
            .lock()
            .unwrap()
            .remove(&job.key)
            .unwrap();
        let _ = reply.send(Some(Arc::new(result)));
    }

    #[cfg(test)]
    pub(in crate::features::services::images) fn waiting(&self) -> usize {
        self.replies
            .lock()
            .unwrap()
            .values()
            .map(watch::Sender::receiver_count)
            .sum()
    }
}
