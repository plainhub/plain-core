use plain_rs::events::WsEvent;
use plain_rs::runtime::Runtime as CoreRuntime;
use std::sync::Arc;
use tokio::sync::watch;

pub struct Capture {
    pub text: String,
    pub sensitive: bool,
    pub eligible: bool,
}

pub struct Changes(watch::Sender<Option<Arc<Capture>>>);

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

    pub fn received(&self, text: &str) {
        self.0.send_if_modified(|current| {
            if current
                .as_ref()
                .is_some_and(|old| old.text == text && !old.eligible && !old.sensitive)
            {
                return false;
            }
            *current = Some(Arc::new(Capture {
                text: text.into(),
                sensitive: false,
                eligible: false,
            }));
            true
        });
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
    db: &plain_rs::db::Db,
    host: &Arc<plain_rs::host::bridge::Host>,
    text: &str,
    source: &str,
    sensitive: bool,
) -> Result<(plain_rs::db::ClipboardRow, bool), String> {
    let result = plain_rs::clipboard::record(db, text, source, "", sensitive)?;
    if result.1 {
        host.invalidate("clipboardHistoryRefresh");
    }
    Ok(result)
}

pub fn report(state: &CoreRuntime, payload: &str) -> Result<(), String> {
    let value: Report = serde_json::from_str(payload).map_err(|error| error.to_string())?;
    if !plain_rs::services::device::access::is_enabled(&state.prefs, &["CLIPBOARD"]) {
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
        let time = chrono::DateTime::parse_from_rfc3339(&item.created_at)
            .map_err(|error| error.to_string())?
            .timestamp_millis();
        let _ = state.events.send(WsEvent::broadcast(
            "CLIPBOARD_CHANGED",
            serde_json::json!({"text": item.text, "sensitive": false, "time": time}).to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/services/clipboard.rs"]
mod tests;
