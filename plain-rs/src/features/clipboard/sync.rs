use crate::features::prefs::Prefs;
use crate::features::prefs::pref_keys::CLIPBOARD_DIRECTIONS;
use plain_libs::anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Direction {
    Off,
    Send,
    #[default]
    Receive,
    Both,
}

impl Direction {
    pub fn sends(self) -> bool {
        matches!(self, Self::Send | Self::Both)
    }

    pub fn receives(self) -> bool {
        matches!(self, Self::Receive | Self::Both)
    }
}

pub fn direction(prefs: &Prefs, peer_id: &str) -> Result<Direction> {
    Ok(prefs
        .get::<BTreeMap<String, plain_libs::serde_json::Value>>(CLIPBOARD_DIRECTIONS)?
        .and_then(|mut directions| directions.remove(peer_id))
        .map(plain_libs::serde_json::from_value)
        .transpose()?
        .unwrap_or_default())
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/features/clipboard/sync.rs"]
mod tests;
