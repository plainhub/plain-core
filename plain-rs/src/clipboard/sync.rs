use crate::prefs::Prefs;
use crate::prefs::pref_keys::CLIPBOARD_DIRECTIONS;
use anyhow::{Result, ensure};
use serde::Deserialize;
use serde_json::Value;

const APPLIED: &str = "clipboard_last_item";

#[derive(Deserialize)]
pub struct Item {
    pub id: String,
    pub text: String,
    pub sensitive: bool,
}

pub fn receives(prefs: &Prefs, peer_id: &str) -> Result<bool> {
    Ok(matches!(
        direction(prefs, peer_id)?.as_str(),
        "pull" | "both"
    ))
}

pub fn sends(prefs: &Prefs, peer_id: &str) -> Result<bool> {
    Ok(matches!(
        direction(prefs, peer_id)?.as_str(),
        "push" | "both"
    ))
}

fn direction(prefs: &Prefs, peer_id: &str) -> Result<String> {
    let directions = prefs
        .get::<serde_json::Map<String, Value>>(CLIPBOARD_DIRECTIONS)?
        .unwrap_or_default();
    match directions.get(peer_id) {
        None => Ok("pull".into()),
        Some(Value::String(direction))
            if matches!(direction.as_str(), "pull" | "both" | "off" | "push") =>
        {
            Ok(direction.clone())
        }
        _ => anyhow::bail!("Invalid clipboard direction"),
    }
}

pub fn prepare(prefs: &Prefs, store: &crate::kv::Db, peer_id: &str, item: &Item) -> Result<bool> {
    if !receives(prefs, peer_id)? || item.sensitive {
        return Ok(false);
    }
    ensure!(!item.id.is_empty(), "Missing clipboard item identity");
    super::validate_text(&item.text).map_err(anyhow::Error::msg)?;
    Ok(
        store.update_json(APPLIED, |map: &mut serde_json::Map<String, Value>| {
            let mut applied: std::collections::BTreeMap<String, String> =
                serde_json::from_value(Value::Object(map.clone()))?;
            match applied.get(peer_id) {
                Some(id) => Ok(id != &item.id),
                None => {
                    applied.insert(peer_id.into(), item.id.clone());
                    *map = serde_json::to_value(applied)?.as_object().unwrap().clone();
                    Ok(false)
                }
            }
        })?,
    )
}

pub fn applied(store: &crate::kv::Db, peer_id: &str, item: &Item) -> Result<()> {
    Ok(
        store.update_json(APPLIED, |map: &mut serde_json::Map<String, Value>| {
            let mut applied: std::collections::BTreeMap<String, String> =
                serde_json::from_value(Value::Object(map.clone()))?;
            applied.insert(peer_id.into(), item.id.clone());
            *map = serde_json::to_value(applied)?.as_object().unwrap().clone();
            Ok(())
        })?,
    )
}

#[cfg(test)]
#[path = "../../tests/unit/clipboard/sync.rs"]
mod tests;
