use crate::infrastructure::db::Db;
use plain_libs::serde_json::Value;
use std::path::Path;

pub fn mount_id(mount: &Value) -> String {
    mount
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("path:{}", mount["path"].as_str().unwrap_or_default()))
}

fn mounted_path(mount: &Value) -> Option<&Path> {
    mount
        .get("mountPoint")
        .unwrap_or(&mount["path"])
        .as_str()
        .map(Path::new)
        .filter(|path| path.is_absolute())
}

pub fn apply_aliases(db: &Db, mut facts: Value) -> Result<Value, String> {
    let mounts = facts.as_array_mut().ok_or("Invalid storage volume facts")?;
    let aliases = db
        .volumes()
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|volume| (volume.identity, volume.alias))
        .collect::<std::collections::HashMap<_, _>>();
    for mount in mounts {
        if mount["driveType"] == "APP" {
            mount["alias"] = Value::String(String::new());
            continue;
        }
        let location = mounted_path(mount)
            .map(crate::infrastructure::db::files::available_location)
            .transpose()
            .map_err(|error| error.to_string())?
            .flatten();
        let alias = location.and_then(|location| aliases.get(&location.volume.identity));
        mount["alias"] = Value::String(alias.cloned().unwrap_or_default());
    }
    Ok(facts)
}

pub fn set_alias(db: &Db, facts: &Value, id: &str, alias: &str) -> Result<(), String> {
    let mount = facts
        .as_array()
        .ok_or("Invalid storage volume facts")?
        .iter()
        .find(|mount| mount_id(mount) == id)
        .ok_or("Mount not found")?;
    if mount["driveType"] == "APP" {
        return Err("App Data cannot be renamed".into());
    }
    let path = mounted_path(mount).ok_or("Mounted filesystem volume required")?;
    let volume_id = db
        .register_volume(path)
        .map_err(|error| error.to_string())?
        .ok_or("Filesystem volume is unavailable")?;
    if !db
        .set_volume_alias(&volume_id, alias)
        .map_err(|error| error.to_string())?
    {
        return Err("Volume not found".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-rs/unit/features/services/files/mounts/aliases.rs"]
mod tests;
