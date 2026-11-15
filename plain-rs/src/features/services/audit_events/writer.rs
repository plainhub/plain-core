use crate::features::audit_events::AuditEvent;
use crate::infrastructure::db::{AuditEventClient, Db};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

type Receipt = oneshot::Sender<Result<(), String>>;
enum Command {
    Record {
        event: AuditEvent,
        client: Option<AuditEventClient>,
        created_at: String,
    },
    Flush(Receipt),
    Stop(Receipt),
}

#[derive(Clone)]
pub struct AuditEvents {
    sender: mpsc::UnboundedSender<Command>,
}

impl AuditEvents {
    pub fn start(db: Arc<Db>) -> Result<Self, String> {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        std::thread::Builder::new()
            .name("audit-events".into())
            .spawn(move || {
                let mut failure = None;
                while let Some(command) = receiver.blocking_recv() {
                    match command {
                        Command::Record {
                            event,
                            client,
                            created_at,
                        } => {
                            if let Err(error) =
                                db.audit_event_add(&event, client.as_ref(), &created_at)
                            {
                                log::error!("Event recording failed: {error}");
                                failure = Some(error.to_string());
                            }
                        }
                        Command::Flush(reply) => {
                            let _ = reply.send(failure.take().map_or(Ok(()), Err));
                        }
                        Command::Stop(reply) => {
                            let _ = reply.send(failure.take().map_or(Ok(()), Err));
                            break;
                        }
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(Self { sender })
    }

    pub fn record(
        &self,
        event: AuditEvent,
        client: Option<AuditEventClient>,
    ) -> Result<(), String> {
        self.sender
            .send(Command::Record {
                event,
                client,
                created_at: plain_libs::chrono::Utc::now()
                    .to_rfc3339_opts(plain_libs::chrono::SecondsFormat::Millis, true),
            })
            .map_err(|_| "Event writer stopped".to_owned())
    }

    pub async fn flush(&self) -> Result<(), String> {
        let (send, receive) = oneshot::channel();
        self.sender
            .send(Command::Flush(send))
            .map_err(|_| "Event writer stopped".to_owned())?;
        receive
            .await
            .map_err(|_| "Event writer stopped".to_owned())?
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        let (send, receive) = oneshot::channel();
        self.sender
            .send(Command::Stop(send))
            .map_err(|_| "Event writer stopped".to_owned())?;
        receive
            .await
            .map_err(|_| "Event writer stopped".to_owned())?
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/audit_events/writer.rs"]
mod tests;
