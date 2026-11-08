const STREAM_BUFFER_MIN: usize = 1024;

pub(crate) const DEFAULT_STREAM_MAX_BUFFER_SIZE: usize = 1024 * 1024;

use std::convert::TryFrom;
use std::io::{self};

/// A buffer for stream data that grows up to a configurable maximum.
///
/// The buffer starts at a minimum size, grows by a fixed factor, and never
/// exceeds the configured max.
pub(crate) struct StreamBuffer {
    data: Vec<u8>,
    pos: usize,
    cap: usize,
}

impl StreamBuffer {
    pub(crate) fn new() -> StreamBuffer {
        StreamBuffer {
            data: vec![0; STREAM_BUFFER_MIN],
            pos: 0,
            cap: 0,
        }
    }

    pub(crate) fn cursor(&self) -> usize {
        self.pos
    }

    pub(crate) fn has_remaining(&self) -> bool {
        self.pos < self.cap
    }

    pub(crate) fn remaining_slice(&self) -> &[u8] {
        &self.data[self.pos..self.cap]
    }

    pub(crate) fn refill_with<F>(&mut self, remaining: u64, fill: F) -> io::Result<()>
    where
        F: FnOnce(&mut [u8]) -> io::Result<usize>,
    {
        self.pos = 0;
        self.grow_for_read_remaining(remaining);
        let cap = fill(&mut self.data)?;
        self.set_cap(cap);
        Ok(())
    }

    fn set_cap(&mut self, cap: usize) {
        debug_assert!(cap <= self.data.len());
        self.cap = cap;
        if self.pos > self.cap {
            self.pos = self.cap;
        }
    }

    /// Clears the filled region and resets the cursor to 0.
    pub(crate) fn clear(&mut self) {
        self.pos = 0;
        self.cap = 0;
    }

    /// Consumes bytes from the filled region, asserting it stays in-bounds.
    pub(crate) fn consume(&mut self, amt: usize) {
        debug_assert!(self.pos + amt <= self.cap);
        self.pos += amt;
    }

    fn grow_for_read_remaining(&mut self, remaining: u64) {
        let current_len = self.data.len() as u64;
        if remaining <= current_len {
            return;
        }
        let remaining_usize = usize::try_from(remaining).unwrap_or(DEFAULT_STREAM_MAX_BUFFER_SIZE);
        let desired = remaining_usize
            .min(DEFAULT_STREAM_MAX_BUFFER_SIZE)
            .max(STREAM_BUFFER_MIN);
        self.data.resize(desired, 0);
    }
}
