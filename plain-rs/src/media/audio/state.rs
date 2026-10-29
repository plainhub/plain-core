use super::playback::Playback;
use crate::db::Db;
use crate::error::{DomainError, DomainResult};
use std::sync::Mutex;

use std::sync::atomic::{AtomicI64, Ordering};

static NEXT_REVISION: AtomicI64 = AtomicI64::new(1);

#[derive(Clone)]
pub(crate) struct State {
    pub playback: Playback,
    pub current_file_id: Option<String>,
    pub load_revision: i64,
    pub started_revision: i64,
}

impl State {
    fn new(path: String) -> Self {
        Self {
            playback: Playback {
                path,
                ..Playback::default()
            },
            current_file_id: None,
            load_revision: -1,
            started_revision: -1,
        }
    }

    pub fn advance(&mut self) -> DomainResult<()> {
        self.playback.revision = NEXT_REVISION
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |revision| {
                revision.checked_add(1)
            })
            .map_err(|_| DomainError::Other("audio revision overflow".into()))?;
        Ok(())
    }

    pub fn select(&mut self, path: &str) -> DomainResult<()> {
        if path.is_empty() {
            self.current_file_id = None;
        }
        if self.playback.path != path {
            self.advance()?;
            self.playback.path = path.into();
            self.playback.position_ms = 0;
            self.load_revision = -1;
            self.started_revision = -1;
        }
        Ok(())
    }
}

pub(crate) struct Runtime {
    state: Mutex<State>,
    restored: Mutex<bool>,
    key: &'static str,
}

impl Runtime {
    pub fn new(key: &'static str, current_file_id: Option<String>) -> Self {
        let mut state = State::new(String::new());
        state.current_file_id = current_file_id;
        Self {
            state: Mutex::new(state),
            restored: Mutex::new(false),
            key,
        }
    }

    pub fn restore_needed(&self) -> bool {
        !*self.restored.lock().unwrap()
    }

    pub fn restore(&self, path: String) -> DomainResult<()> {
        let mut restored = self.restored.lock().unwrap();
        if !*restored {
            let mut state = self.state.lock().unwrap();
            if state.playback.path.is_empty() {
                state.select(&path)?;
            }
            *restored = true;
        }
        Ok(())
    }

    pub fn read<T>(&self, read: impl FnOnce(&State) -> T) -> T {
        read(&self.state.lock().unwrap())
    }

    pub fn update<T>(&self, update: impl FnOnce(&mut State) -> DomainResult<T>) -> DomainResult<T> {
        let mut state = self.state.lock().unwrap();
        let mut next = state.clone();
        let result = update(&mut next)?;
        *state = next;
        Ok(result)
    }

    pub fn transaction<T>(
        &self,
        db: &Db,
        operation: impl FnOnce(&rusqlite::Transaction<'_>, &mut State) -> DomainResult<T>,
    ) -> DomainResult<T> {
        let mut state = self.state.lock().unwrap();
        let mut next = state.clone();
        let result = db.with_conn(|connection| -> DomainResult<T> {
            let transaction = connection.unchecked_transaction()?;
            let result = operation(&transaction, &mut next)?;
            if next.current_file_id != state.current_file_id {
                crate::kv::set_in_connection(
                    &transaction,
                    self.key,
                    next.current_file_id.as_deref(),
                )?;
            }
            transaction.commit()?;
            Ok(result)
        })?;
        *state = next;
        Ok(result)
    }
}
