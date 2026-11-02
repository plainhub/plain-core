use rusqlite::Connection;
use std::sync::Mutex;

#[cfg(all(feature = "chat", feature = "library"))]
pub(super) struct SessionSnapshot {
    pub(super) revision: u64,
    pub(super) rows: std::collections::HashMap<String, super::SessionRow>,
    pub(super) activity: plain_libs::utils::write_batch::WriteBatch<(String, String)>,
}

pub(super) struct State {
    #[cfg(all(feature = "chat", feature = "library"))]
    pub(super) sessions: std::sync::RwLock<SessionSnapshot>,
    #[cfg(all(feature = "chat", feature = "library"))]
    pub(super) session_revision: std::sync::Arc<std::sync::atomic::AtomicU64>,
    pub(super) app_files: Mutex<()>,
    #[cfg(feature = "chat")]
    pub(super) peers: tokio::sync::watch::Sender<u64>,
}

impl State {
    pub(super) fn new(_connection: &mut Connection) -> rusqlite::Result<Self> {
        #[cfg(all(feature = "chat", feature = "library"))]
        _connection.authorizer(Some(|context: rusqlite::hooks::AuthContext<'_>| {
            // Preserve DELETE row callbacks for session-cache revocation: https://sqlite.org/lang_delete.html#the_truncate_optimization
            if context.database_name == Some("main")
                && matches!(
                    context.action,
                    rusqlite::hooks::AuthAction::Delete {
                        table_name: "sessions"
                    }
                )
            {
                rusqlite::hooks::Authorization::Ignore
            } else {
                rusqlite::hooks::Authorization::Allow
            }
        }))?;
        #[cfg(all(feature = "chat", feature = "library"))]
        let session_revision = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        #[cfg(all(feature = "chat", feature = "library"))]
        let sessions = std::sync::RwLock::new(SessionSnapshot {
            revision: 0,
            activity: Default::default(),
            rows: super::repositories::simple::session_snapshot(_connection)?,
        });
        #[cfg(feature = "chat")]
        let peers = {
            let sender = tokio::sync::watch::channel(0u64).0;
            let changes = sender.clone();
            #[cfg(feature = "library")]
            let session_revision = session_revision.clone();
            _connection.update_hook(Some(move |_, database: &str, table: &str, _| {
                #[cfg(feature = "library")]
                if database == "main" && table == "sessions" {
                    session_revision.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                }
                if database == "main" && table == "peers" {
                    changes.send_modify(|revision| *revision = revision.wrapping_add(1));
                }
            }))?;
            sender
        };
        Ok(Self {
            app_files: Mutex::new(()),
            #[cfg(all(feature = "chat", feature = "library"))]
            sessions,
            #[cfg(all(feature = "chat", feature = "library"))]
            session_revision,
            #[cfg(feature = "chat")]
            peers,
        })
    }
}

#[cfg(all(test, feature = "chat"))]
#[path = "../../../../tests/plain-rs/unit/infrastructure/db/state.rs"]
mod tests;
