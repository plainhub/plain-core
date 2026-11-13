use super::*;
use crate::features::downloads::{Source, queue::Run};
use tokio::sync::mpsc::UnboundedSender;

type Finished = (String, String);
struct Completion {
    value: Finished,
    sender: UnboundedSender<Finished>,
}
impl Drop for Completion {
    fn drop(&mut self) {
        let _ = self.sender.send(self.value.clone());
    }
}

pub(super) struct Worker {
    pub generation: String,
    pub job: JoinHandle<Result<()>>,
    stop: watch::Sender<bool>,
}
impl Worker {
    pub fn start(state: CoreRuntime, run: Run, sender: UnboundedSender<Finished>) -> Self {
        let generation = run.generation.clone();
        let (cancel, mut canceled) = watch::channel(false);
        let job = tokio::spawn(async move {
            let _completion = Completion {
                value: (run.download.id.clone(), run.generation.clone()),
                sender,
            };
            let io = super::super::io::BlockingWork::default();
            let canceled_io = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let cancel_io = super::super::archive::Cancel(canceled_io.clone());
            let mut work = Box::pin(async {
                match &run.download.source {
                    Source::Chat { .. } => super::super::chat::execute(&state, &run, &io).await,
                    Source::Resources { .. } => {
                        super::super::resources::execute(&state, &run, &io, canceled_io.clone())
                            .await
                    }
                }
            });
            let result = tokio::select! { _ = canceled.changed() => None, result = &mut work => Some(result) };
            drop(cancel_io);
            let drained = io.finish().await;
            drop(work);
            if let Some(result) = result {
                let result = match (result, drained) {
                    (Err(error), Err(cleanup)) => Err(plain_libs::anyhow::anyhow!(
                        "{error}; download cleanup failed: {cleanup}"
                    )),
                    (result, Ok(())) => result,
                    (Ok(()), Err(error)) => Err(error),
                };
                state
                    .downloads
                    .finish(&run.download.id, &run.generation, result)?;
            } else {
                drained?;
            }
            Ok(())
        });
        Self {
            generation,
            job,
            stop: cancel,
        }
    }
    pub async fn stop(self) -> Result<()> {
        self.stop.send_replace(true);
        self.job.await?
    }
}
