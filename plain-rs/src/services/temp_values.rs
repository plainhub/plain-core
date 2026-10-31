use std::{collections::HashMap, sync::Mutex};

/// Process-lifetime scratch storage for values a client hands over one
/// request at a time. `setTempValue` writes an archive selection here and
/// `/zip/files` consumes it under the key the client was given.
#[derive(Clone, Default)]
pub struct TempValues {
    entries: std::sync::Arc<Mutex<HashMap<String, String>>>,
}

impl TempValues {
    pub fn set(&self, key: String, value: String) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.insert(key, value);
        }
    }

    /// Reads a value out and forgets it, so a key cannot be replayed into a
    /// second archive.
    pub fn take(&self, key: &str) -> Option<String> {
        self.entries.lock().ok()?.remove(key)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/services/temp_values.rs"]
mod tests;
