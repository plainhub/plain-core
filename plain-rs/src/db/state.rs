use rusqlite::Connection;
use std::sync::Mutex;

pub(super) struct State {
    pub(super) app_files: Mutex<()>,
    #[cfg(feature = "chat")]
    pub(super) peers: tokio::sync::watch::Sender<u64>,
}

impl State {
    pub(super) fn new(_connection: &mut Connection) -> rusqlite::Result<Self> {
        #[cfg(feature = "chat")]
        let peers = {
            let sender = tokio::sync::watch::channel(0u64).0;
            let changes = sender.clone();
            _connection.update_hook(Some(move |_, database: &str, table: &str, _| {
                if database == "main" && table == "peers" {
                    changes.send_modify(|revision| *revision = revision.wrapping_add(1));
                }
            }))?;
            sender
        };
        Ok(Self {
            app_files: Mutex::new(()),
            #[cfg(feature = "chat")]
            peers,
        })
    }
}

#[cfg(all(test, feature = "chat"))]
#[path = "../../tests/unit/db/state.rs"]
mod tests;
