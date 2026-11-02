use std::time::{Duration, Instant};

pub struct WriteBatch<T> {
    pending: Vec<T>,
    since: Option<Instant>,
    limit: usize,
    delay: Duration,
}

impl<T> Default for WriteBatch<T> {
    fn default() -> Self {
        Self::new(50, Duration::from_secs(60))
    }
}

impl<T> WriteBatch<T> {
    pub fn new(limit: usize, delay: Duration) -> Self {
        Self {
            pending: Vec::new(),
            since: None,
            limit,
            delay,
        }
    }

    pub fn push(&mut self, value: T, now: Instant) {
        self.since.get_or_insert(now);
        self.pending.push(value);
    }

    pub fn pending(&self) -> &[T] {
        &self.pending
    }

    pub fn take_pending(&mut self) -> Self {
        Self {
            pending: std::mem::take(&mut self.pending),
            since: self.since.take(),
            limit: self.limit,
            delay: self.delay,
        }
    }

    pub fn restore_pending(&mut self, mut older: Self) {
        older.pending.append(&mut self.pending);
        self.pending = older.pending;
        self.since = match (older.since, self.since) {
            (Some(old), Some(new)) => Some(old.min(new)),
            (old, new) => old.or(new),
        };
    }

    pub fn is_due(&self, now: Instant) -> bool {
        self.pending.len() > self.limit
            || self
                .since
                .is_some_and(|since| now.saturating_duration_since(since) >= self.delay)
    }

    pub fn retain(&mut self, keep: impl FnMut(&T) -> bool) {
        self.pending.retain(keep);
        if self.pending.is_empty() {
            self.since = None;
        }
    }

    pub fn flush_due<E>(
        &mut self,
        now: Instant,
        write: impl FnOnce(&[T]) -> Result<(), E>,
    ) -> Result<bool, E> {
        if !self.is_due(now) {
            return Ok(false);
        }
        self.flush(write)
    }

    pub fn flush<E>(&mut self, write: impl FnOnce(&[T]) -> Result<(), E>) -> Result<bool, E> {
        if self.pending.is_empty() {
            return Ok(false);
        }
        write(&self.pending)?;
        self.pending.clear();
        self.since = None;
        Ok(true)
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/utils/write_batch.rs"]
mod tests;
