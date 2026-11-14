use crate::features::enums::{DataType, MediaDataType};
use crate::infrastructure::events::MEDIA_UPDATED;
use crate::infrastructure::events::WsEvent;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use tokio::sync::broadcast;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Change {
    types: Vec<MediaDataType>,
}

pub fn includes(payload: &str, kind: DataType) -> bool {
    MediaDataType::from_data_type(kind).is_some_and(|kind| {
        plain_libs::serde_json::from_str::<Change>(payload).is_ok_and(|change| change.types.contains(&kind))
    })
}

pub fn publish(events: &broadcast::Sender<WsEvent>, types: impl IntoIterator<Item = i32>) {
    let types = types
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter_map(|kind| DataType::from_kind(kind).and_then(MediaDataType::from_data_type))
        .collect::<Vec<_>>();
    if !types.is_empty() {
        let _ = events.send(WsEvent::broadcast(
            MEDIA_UPDATED,
            plain_libs::serde_json::to_string(&Change { types }).unwrap(),
        ));
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/files/catalog_events.rs"]
mod tests;
