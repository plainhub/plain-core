use super::Prefs;
use crate::prefs::pref_keys::VOLUME_ALIAS;
use anyhow::Result;
use serde_json::{Map, Value};

pub fn get_map(prefs: &Prefs) -> Map<String, Value> {
    prefs
        .get::<Map<String, Value>>(VOLUME_ALIAS)
        .ok()
        .flatten()
        .unwrap_or_default()
}

pub fn set_map(prefs: &Prefs, m: &Map<String, Value>) -> Result<()> {
    prefs.set(VOLUME_ALIAS, m)?;
    Ok(())
}

pub fn set_alias(prefs: &Prefs, id: &str, alias: &str) -> Result<()> {
    let mut m = get_map(prefs);
    let trimmed = alias.trim();
    if trimmed.is_empty() {
        m.remove(id);
    } else {
        m.insert(id.to_string(), Value::String(trimmed.to_string()));
    }
    set_map(prefs, &m)
}
