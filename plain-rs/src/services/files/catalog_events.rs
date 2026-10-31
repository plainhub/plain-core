use plain_rs::events::{MEDIA_LIBRARY_UPDATED, WsEvent};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use tokio::sync::broadcast;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Change {
    types: Vec<i32>,
}

pub fn includes(payload: &str, kind: plain_rs::enums::DataType) -> bool {
    serde_json::from_str::<Change>(payload).is_ok_and(|change| change.types.contains(&kind.kind()))
}

pub fn publish(events: &broadcast::Sender<WsEvent>, types: impl IntoIterator<Item = i32>) {
    let types = types
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if !types.is_empty() {
        let _ = events.send(WsEvent::broadcast(
            MEDIA_LIBRARY_UPDATED,
            serde_json::to_string(&Change { types }).unwrap(),
        ));
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/services/files/catalog_events.rs"]
mod tests;
