use super::{CoreRuntime, Request, Thumbnails};
use futures_util::{StreamExt, stream::FuturesUnordered};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::sync::{mpsc, watch};

type Output = Result<Option<bytes::Bytes>, String>;
type Reply = Option<Arc<Output>>;

pub(crate) struct Generation {
    pub(super) request: Request,
    pub(super) cache: PathBuf,
    pub(super) key: String,
    pub(super) cached: PathBuf,
}

struct Work {
    job: Mutex<Option<Generation>>,
    reply: watch::Sender<Reply>,
}

struct Entry {
    work: Arc<Work>,
    foreground: bool,
}

pub(crate) struct Receivers {
    foreground: mpsc::Receiver<Arc<Work>>,
    background: mpsc::Receiver<Arc<Work>>,
}

impl Receivers {
    async fn recv(&mut self) -> Option<Arc<Work>> {
        tokio::select! {
            biased;
            work = self.foreground.recv() => work,
            work = self.background.recv() => work,
        }
    }
}

pub(in crate::features::services::images) struct Queue {
    foreground: mpsc::Sender<Arc<Work>>,
    background: mpsc::Sender<Arc<Work>>,
    replies: Mutex<HashMap<String, Entry>>,
}

impl Queue {
    pub(super) fn new() -> (Self, Receivers) {
        let (foreground, foreground_receiver) = mpsc::channel(320);
        let (background, background_receiver) = mpsc::channel(320);
        (
            Self {
                foreground,
                background,
                replies: Mutex::new(HashMap::new()),
            },
            Receivers {
                foreground: foreground_receiver,
                background: background_receiver,
            },
        )
    }

    pub(super) async fn submit(
        &self,
        request: Request,
        cache: PathBuf,
        key: String,
        cached: PathBuf,
        background: bool,
    ) -> Output {
        let sender = if background {
            &self.background
        } else {
            &self.foreground
        };
        let permit = sender.reserve().await.map_err(|_| "Rust core stopped")?;
        let mut reply = {
            let mut replies = self.replies.lock().unwrap();
            if let Some(entry) = replies.get_mut(&key) {
                let reply = entry.work.reply.subscribe();
                if !background && !entry.foreground {
                    entry.foreground = true;
                    permit.send(entry.work.clone());
                } else {
                    drop(permit);
                }
                reply
            } else {
                let (tx, rx) = watch::channel(None);
                let work = Arc::new(Work {
                    job: Mutex::new(Some(Generation {
                        request,
                        cache,
                        key: key.clone(),
                        cached,
                    })),
                    reply: tx,
                });
                replies.insert(
                    key,
                    Entry {
                        work: work.clone(),
                        foreground: !background,
                    },
                );
                permit.send(work);
                rx
            }
        };
        reply.changed().await.map_err(|_| "Rust core stopped")?;
        let result = reply.borrow_and_update().as_ref().unwrap().clone();
        drop(reply);
        Arc::unwrap_or_clone(result)
    }

    pub(super) async fn run(state: CoreRuntime, mut receivers: Receivers) {
        let workers = state.thumbnails.workers();
        let mut stop = state.stop.clone();
        let mut running = FuturesUnordered::new();
        loop {
            let work = tokio::select! {
                biased;
                _ = stop.wait_for(|value| *value) => break,
                _ = running.next(), if !running.is_empty() => continue,
                work = receivers.recv(), if running.len() < workers => work,
            };
            let Some(work) = work else {
                break;
            };
            let mut replies = state.thumbnails.queue.replies.lock().unwrap();
            let job = work.job.lock().unwrap().take();
            if let Some(job) = job {
                if work.reply.receiver_count() == 0 {
                    replies.remove(&job.key);
                } else {
                    running.push(Self::complete(&state, job));
                }
            }
        }
        drop(receivers);
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
        let _ = reply.work.reply.send(Some(Arc::new(result)));
    }

    #[cfg(test)]
    pub(in crate::features::services::images) fn waiting(&self) -> usize {
        self.replies
            .lock()
            .unwrap()
            .values()
            .map(|entry| entry.work.reply.receiver_count())
            .sum()
    }
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-rs/unit/features/services/images/thumbnails/queue.rs"]
mod tests;
