use super::{CoreRuntime, Request, Thumbnails};
use futures_util::{StreamExt, stream::FuturesUnordered};
use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::sync::{Notify, watch};

type Output = Result<Option<bytes::Bytes>, String>;
type Reply = Option<Arc<Output>>;

pub(crate) struct Generation {
    pub(super) request: Request,
    pub(super) cache: PathBuf,
    pub(super) key: String,
    pub(super) cached: PathBuf,
}

struct Entry {
    reply: watch::Sender<Reply>,
    queued_order: Option<u64>,
}

#[derive(Default)]
struct Pending {
    jobs: BTreeMap<u64, Generation>,
    replies: HashMap<String, Entry>,
}

#[derive(Default)]
pub(in crate::features::services::images) struct Queue {
    pending: Mutex<Pending>,
    ready: Notify,
}

struct Waiting<'a> {
    queue: &'a Queue,
    key: String,
    receiver: Option<watch::Receiver<Reply>>,
}

impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        let receiver = self.receiver.take().unwrap();
        let mut pending = self.queue.pending.lock().unwrap();
        let Some(entry) = pending.replies.get(&self.key) else {
            return;
        };
        let same_channel = receiver.same_channel(&entry.reply.subscribe());
        drop(receiver);
        if same_channel && entry.reply.receiver_count() == 0 {
            if let Some(order) = entry.queued_order {
                pending.jobs.remove(&order);
                pending.replies.remove(&self.key);
            }
        }
    }
}

impl Queue {
    pub(super) async fn submit(&self, job: Generation, access_order: u64) -> Output {
        let key = job.key.clone();
        let receiver = {
            let mut pending = self.pending.lock().unwrap();
            let Pending { jobs, replies } = &mut *pending;
            if let Some(entry) = replies.get_mut(&key) {
                if let Some(previous) = entry
                    .queued_order
                    .filter(|previous| *previous < access_order)
                {
                    let previous_job = jobs.remove(&previous).unwrap();
                    jobs.insert(access_order, previous_job);
                    entry.queued_order = Some(access_order);
                }
                drop(job);
                entry.reply.subscribe()
            } else {
                let (sender, receiver) = watch::channel(None);
                replies.insert(
                    key.clone(),
                    Entry {
                        reply: sender,
                        queued_order: Some(access_order),
                    },
                );
                jobs.insert(access_order, job);
                receiver
            }
        };
        let mut waiting = Waiting {
            queue: self,
            key,
            receiver: Some(receiver),
        };
        self.ready.notify_one();
        let reply = waiting.receiver.as_mut().unwrap();
        reply.changed().await.map_err(|_| "Rust core stopped")?;
        let result = reply.borrow_and_update().as_ref().unwrap().clone();
        drop(waiting);
        Arc::unwrap_or_clone(result)
    }

    pub(super) async fn run(state: CoreRuntime) {
        let workers = state.thumbnails.workers();
        let mut stop = state.stop.clone();
        let mut running = FuturesUnordered::new();
        loop {
            let ready = state.thumbnails.queue.ready.notified();
            tokio::pin!(ready);
            ready.as_mut().enable();
            if *stop.borrow() {
                break;
            }
            if running.len() < workers {
                let mut pending = state.thumbnails.queue.pending.lock().unwrap();
                if let Some((_, job)) = pending.jobs.pop_last() {
                    let entry = pending.replies.get_mut(&job.key).unwrap();
                    entry.queued_order = None;
                    if entry.reply.receiver_count() == 0 {
                        pending.replies.remove(&job.key);
                    } else {
                        running.push(Self::complete(&state, job));
                    }
                    continue;
                }
            }
            tokio::select! {
                _ = stop.wait_for(|value| *value) => break,
                _ = running.next(), if !running.is_empty() => {},
                _ = ready => {},
            }
        }
        while running.next().await.is_some() {}
        let mut pending = state.thumbnails.queue.pending.lock().unwrap();
        pending.jobs.clear();
        pending.replies.clear();
    }

    async fn complete(state: &CoreRuntime, job: Generation) {
        let result = Thumbnails::generate(state, &job).await;
        let reply = state
            .thumbnails
            .queue
            .pending
            .lock()
            .unwrap()
            .replies
            .remove(&job.key)
            .unwrap();
        let _ = reply.reply.send(Some(Arc::new(result)));
    }

    #[cfg(any(test, feature = "test-seams"))]
    pub(in crate::features::services::images) fn waiting(&self) -> usize {
        self.pending
            .lock()
            .unwrap()
            .replies
            .values()
            .map(|entry| entry.reply.receiver_count())
            .sum()
    }
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-rs/unit/features/services/images/thumbnails/queue.rs"]
mod tests;
