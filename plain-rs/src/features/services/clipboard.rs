use crate::features::access::AccessFeature;
use crate::infrastructure::events::WsEvent;
use crate::runtime::Runtime as CoreRuntime;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
#[derive(Default)]
pub struct ActiveWrites(Mutex<HashMap<String, usize>>);

pub struct Receipt<'a> {
    writes: &'a ActiveWrites,
    text: &'a str,
}

impl Drop for Receipt<'_> {
    fn drop(&mut self) {
        let mut writes = self.writes.0.lock().unwrap();
        if let Some(count) = writes.get_mut(self.text) {
            *count -= 1;
            if *count == 0 {
                writes.remove(self.text);
            }
        }
    }
}

impl ActiveWrites {
    pub fn begin<'a>(&'a self, text: &'a str) -> Receipt<'a> {
        *self.0.lock().unwrap().entry(text.to_owned()).or_default() += 1;
        Receipt { writes: self, text }
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Report {
    text: String,
    sensitive: bool,
}

pub fn record_history(
    db: &crate::infrastructure::db::Db,
    host: &Arc<crate::infrastructure::host::bridge::Host>,
    text: &str,
    source: &str,
    sensitive: bool,
) -> crate::infrastructure::error::DomainResult<(crate::infrastructure::db::ClipboardRow, bool)> {
    let result = crate::features::clipboard::record(db, text, source, "", sensitive)?;
    if result.1 {
        host.invalidate("clipboardHistoryRefresh");
    }
    Ok(result)
}

pub fn record_write(
    db: &crate::infrastructure::db::Db,
    host: &Arc<crate::infrastructure::host::bridge::Host>,
    text: &str,
    source: &str,
    label: &str,
    sensitive: bool,
) -> crate::infrastructure::error::DomainResult<()> {
    let (_, inserted) = crate::features::clipboard::record_write(db, text, source, label, sensitive)?;
    if inserted {
        host.invalidate("clipboardHistoryRefresh");
    }
    Ok(())
}

pub fn report(state: &CoreRuntime, payload: &str) -> Result<(), String> {
    let value: Report = plain_libs::serde_json::from_str(payload).map_err(|error| error.to_string())?;
    if !crate::features::services::device::access::is_enabled(&state.prefs, &[AccessFeature::Clipboard]) {
        return Ok(());
    }
    if state.clipboard_writes.0.lock().unwrap().contains_key(&value.text) {
        return Ok(());
    }
    let (item, inserted) =
        record_history(&state.db, &state.host, &value.text, "", value.sensitive).map_err(|error| error.to_string())?;
    if !inserted {
        return Ok(());
    }
    if !item.sensitive {
        let time = plain_libs::chrono::DateTime::parse_from_rfc3339(&item.created_at)
            .map_err(|error| error.to_string())?
            .timestamp_millis();
        let _ = state.events.send(WsEvent::broadcast(
            "CLIPBOARD_CHANGED",
            plain_libs::serde_json::json!({"text": item.text, "sensitive": false, "time": time}).to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/features/services/clipboard.rs"]
mod tests;
