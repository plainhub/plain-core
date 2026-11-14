use crate::features::access::AccessFeature;
use crate::infrastructure::events::WsEvent;
use crate::runtime::Runtime as CoreRuntime;
use std::sync::Arc;
use tokio::sync::watch;

#[derive(Clone)]
pub struct Capture {
    pub text: String,
    pub sensitive: bool,
    pub eligible: bool,
    received: bool,
}

pub struct Changes(watch::Sender<Option<Arc<Capture>>>);

pub struct Receipt<'a> {
    changes: &'a Changes,
    text: &'a str,
}

impl Drop for Receipt<'_> {
    fn drop(&mut self) {
        self.changes.0.send_if_modified(|current| {
            if let Some(capture) = current.as_mut().filter(|capture| capture.text == self.text) {
                Arc::make_mut(capture).received = false;
            }
            false
        });
    }
}

impl Default for Changes {
    fn default() -> Self {
        Self(watch::channel(None).0)
    }
}

impl Changes {
    pub fn subscribe(&self) -> watch::Receiver<Option<Arc<Capture>>> {
        self.0.subscribe()
    }

    pub fn can_send(&self, text: &str) -> bool {
        self.0
            .borrow()
            .as_ref()
            .is_some_and(|capture| capture.eligible && !capture.sensitive && capture.text == text)
    }

    pub fn received<'a>(&'a self, text: &'a str) -> Receipt<'a> {
        self.0.send_if_modified(|current| {
            if current
                .as_ref()
                .is_some_and(|old| old.text == text && old.received)
            {
                return false;
            }
            *current = Some(Arc::new(Capture {
                text: text.into(),
                sensitive: false,
                eligible: false,
                received: true,
            }));
            true
        });
        Receipt { changes: self, text }
    }

    fn observe(&self, text: &str, sensitive: bool, inserted: bool) {
        self.0.send_if_modified(|current| {
            if !inserted
                && current
                    .as_ref()
                    .is_some_and(|old| old.text == text && old.sensitive == sensitive)
            {
                return false;
            }
            *current = Some(Arc::new(Capture {
                text: text.into(),
                sensitive,
                eligible: inserted && !sensitive,
                received: false,
            }));
            true
        });
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
) -> Result<(crate::infrastructure::db::ClipboardRow, bool), String> {
    let result = crate::features::clipboard::record(db, text, source, "", sensitive)?;
    if result.1 {
        host.invalidate("clipboardHistoryRefresh");
    }
    Ok(result)
}

pub fn report(state: &CoreRuntime, payload: &str) -> Result<(), String> {
    let value: Report = plain_libs::serde_json::from_str(payload).map_err(|error| error.to_string())?;
    if !crate::features::services::device::access::is_enabled(&state.prefs, &[AccessFeature::Clipboard]) {
        return Ok(());
    }
    if !value.sensitive
        && state.clipboard_changes.0.borrow().as_ref().is_some_and(|capture| {
            capture.received && capture.text == value.text
        })
    {
        return Ok(());
    }
    let (item, inserted) =
        record_history(&state.db, &state.host, &value.text, "", value.sensitive)?;
    state
        .clipboard_changes
        .observe(&value.text, value.sensitive, inserted);
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
