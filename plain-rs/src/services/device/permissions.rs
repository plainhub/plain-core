use plain_rs::host::bridge::Host;
use plain_rs::prefs::{Prefs, pref_keys::API_PERMISSIONS};
use plain_rs::services::device::permission::Permission;
use serde_json::{Value, json};

pub async fn granted(host: &Host) -> Result<Vec<String>, String> {
    let names: Vec<&str> = <Permission as async_graphql::EnumType>::items()
        .iter()
        .map(|item| item.name)
        .collect();
    let sample = host
        .call("systemPermissionFacts", json!({"permissions": names}))
        .await?;
    let grants = sample["granted"]
        .as_object()
        .ok_or("Invalid permission sample")?;
    let mut granted = Vec::new();
    for name in names {
        match grants.get(name).and_then(Value::as_bool) {
            Some(true) => granted.push(name.to_owned()),
            Some(false) => {}
            None => return Err(format!("Missing permission sample: {name}")),
        }
    }
    Ok(granted)
}

pub fn enabled(prefs: &Prefs, grants: &[String]) -> Result<Vec<String>, String> {
    let configured: Vec<String> = prefs
        .get(API_PERMISSIONS)
        .map_err(|error| error.to_string())?
        .unwrap_or_default();
    let mut permissions: Vec<String> = grants
        .iter()
        .filter(|name| configured.contains(name))
        .cloned()
        .collect();
    for name in ["RECORD_AUDIO", "ADB"] {
        if grants.iter().any(|grant| grant == name)
            && !permissions.iter().any(|permission| permission == name)
        {
            permissions.push(name.to_owned());
        }
    }
    Ok(permissions)
}
