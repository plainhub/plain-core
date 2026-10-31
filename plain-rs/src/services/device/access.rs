use crate::prefs::Prefs;
use plain_rs::host::bridge::Host;
use serde_json::{Value, json};
use std::collections::HashSet;
/// `WRITE_CONTACTS` implies `READ_CONTACTS` and `WRITE_CALL_LOG` implies
/// `READ_CALL_LOG`: the web UI only ever offers the wider permission, so
/// asking for the narrower one must not fail.
pub fn api_enabled(permissions: &[String], configured: &HashSet<String>) -> bool {
    permissions.iter().all(|name| {
        configured.contains(name)
            || (name == "READ_CONTACTS" && configured.contains("WRITE_CONTACTS"))
            || (name == "READ_CALL_LOG" && configured.contains("WRITE_CALL_LOG"))
    })
}

fn configured(prefs: &Prefs) -> Vec<String> {
    prefs.get_or(plain_rs::prefs::pref_keys::API_PERMISSIONS, Vec::new())
}

pub async fn granted(host: &Host, permissions: &[String]) -> anyhow::Result<bool> {
    let facts = host
        .call(
            "systemPermissionFacts",
            json!({ "permissions": permissions }),
        )
        .await
        .map_err(anyhow::Error::msg)?;
    let grants = facts["granted"]
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("invalid permission facts"))?;
    Ok(permissions
        .iter()
        .all(|name| grants.get(name).and_then(Value::as_bool) == Some(true)))
}

pub fn is_enabled(prefs: &Prefs, permissions: &[&str]) -> bool {
    let names: Vec<String> = permissions.iter().map(|name| name.to_string()).collect();
    let configured: HashSet<String> = configured(prefs).into_iter().collect();
    api_enabled(&names, &configured)
}

#[cfg(test)]
#[path = "../../../tests/unit/services/device/access.rs"]
mod tests;
