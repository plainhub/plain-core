mod command;
use super::pref_keys as keys;
mod update;
pub use command::Command;
pub use update::{ReleaseInfo, UpdateInfo, UpdatePatch};

use super::{Prefs, PrefsError, Result};
use keys::*;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PasswordType {
    None,
    Fixed,
}

const DEFAULT_PASSWORD_TYPE: PasswordType = PasswordType::None;
const DEFAULT_LOGIN_REQUIRE_APPROVAL: bool = true;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityState {
    pub password: String,
    pub password_type: PasswordType,
    pub login_require_approval: bool,
    pub rotate_url_token_on_restart: bool,
    pub adb_token: String,
    pub url_token: String,
    pub api_permissions: Vec<String>,
    pub client_id: String,
    pub mdns_hostname: String,
    pub signature_public_key: String,
}

fn read<T: serde::de::DeserializeOwned + Default>(
    map: &Map<String, Value>,
    key: &str,
) -> Result<T> {
    map.get(key)
        .map(|v| serde_json::from_value(v.clone()).map_err(PrefsError::Decode))
        .unwrap_or_else(|| Ok(T::default()))
}
fn put<T: Serialize>(map: &mut Map<String, Value>, key: &str, value: T) -> Result<()> {
    map.insert(
        key.into(),
        serde_json::to_value(value).map_err(PrefsError::Encode)?,
    );
    Ok(())
}
fn ensure(map: &mut Map<String, Value>, key: &str, make: impl FnOnce() -> String) -> Result<()> {
    if read::<String>(map, key)?.is_empty() {
        put(map, key, make())?;
    }
    Ok(())
}
pub fn random_password(length: usize) -> String {
    use rand::Rng;
    let alphabet = b"23456789abcdefghijkmnpqrstuvwxyzABCDEFGHJKMNPQRSTUVWXYZ";
    let mut rng = rand::thread_rng();
    (0..length)
        .map(|_| alphabet[rng.gen_range(0..alphabet.len())] as char)
        .collect()
}

pub fn bootstrap(prefs: &Prefs) -> Result<()> {
    prefs.update(|map| {
        ensure(map, CLIENT_ID, crate::short_uuid::short_uuid)?;
        ensure(map, SIGNATURE_KEY_PAIR, || {
            crate::base64_encode(&crate::ed25519_generate().0)
        })?;
        ensure(map, MASTER_SECRET, crate::gen_token)?;
        ensure(map, PASSWORD, || random_password(6))?;
        ensure(map, ADB_TOKEN, || random_password(32))?;
        ensure(map, MDNS_HOSTNAME, generate_mdns_hostname)?;
        let rotate = read::<bool>(map, ROTATE_URL_TOKEN_ON_RESTART)?;
        if rotate {
            put(map, URL_TOKEN, crate::gen_token())?;
        } else {
            ensure(map, URL_TOKEN, crate::gen_token)?;
        }
        if !map.contains_key(PASSWORD_TYPE) {
            put(map, PASSWORD_TYPE, DEFAULT_PASSWORD_TYPE)?;
        }
        if !map.contains_key(LOGIN_REQUIRE_APPROVAL) {
            put(map, LOGIN_REQUIRE_APPROVAL, DEFAULT_LOGIN_REQUIRE_APPROVAL)?;
        }
        runtime_defaults(map)?;
        Ok(())
    })
}

fn runtime_defaults(map: &mut Map<String, Value>) -> Result<()> {
    if !map.contains_key(DEV_TOKEN) {
        put(map, DEV_TOKEN, "")?;
    }
    if !map.contains_key(LOG_LEVEL) {
        put(map, LOG_LEVEL, "info")?;
    }
    Ok(())
}

pub fn password_type(prefs: &Prefs) -> Result<PasswordType> {
    Ok(prefs.get(PASSWORD_TYPE)?.unwrap_or(DEFAULT_PASSWORD_TYPE))
}

pub fn dev_token(prefs: &Prefs) -> Result<String> {
    Ok(prefs.get(DEV_TOKEN)?.unwrap_or_default())
}

pub fn log_level(prefs: &Prefs) -> anyhow::Result<log::LevelFilter> {
    let value = prefs
        .get::<String>(LOG_LEVEL)?
        .unwrap_or_else(|| "info".into());
    parse_log_level(&value)
}

fn parse_log_level(value: &str) -> anyhow::Result<log::LevelFilter> {
    match value {
        "trace" => Ok(log::LevelFilter::Trace),
        "debug" => Ok(log::LevelFilter::Debug),
        "info" => Ok(log::LevelFilter::Info),
        "warn" => Ok(log::LevelFilter::Warn),
        "error" => Ok(log::LevelFilter::Error),
        _ => anyhow::bail!("Invalid log level"),
    }
}

pub fn set_entry(prefs: &Prefs, key: &str, value: Value) -> anyhow::Result<bool> {
    if key == AUTO_CHECK_UPDATE {
        anyhow::ensure!(value.is_boolean(), "Invalid auto check update setting");
    }
    if key == SKIP_VERSION {
        anyhow::ensure!(value.is_string(), "Invalid skipped version");
    }
    let command = match key {
        PASSWORD => Some(Command::SetPassword {
            password: serde_json::from_value(value.clone())?,
        }),
        PASSWORD_TYPE => Some(Command::SetPasswordType {
            value: serde_json::from_value(value.clone())?,
        }),
        LOGIN_REQUIRE_APPROVAL => Some(Command::SetLoginRequireApproval {
            enabled: serde_json::from_value(value.clone())?,
        }),
        ROTATE_URL_TOKEN_ON_RESTART => Some(Command::SetRotateUrlToken {
            enabled: serde_json::from_value(value.clone())?,
        }),
        MDNS_HOSTNAME => Some(Command::SetMdnsHostname {
            hostname: serde_json::from_value(value.clone())?,
        }),
        API_PERMISSIONS if value.is_object() => Some(Command::PatchApiPermissions {
            permissions: serde_json::from_value(value.clone())?,
        }),
        API_PERMISSIONS => Some(Command::SetApiPermissions {
            permissions: serde_json::from_value(value.clone())?,
        }),
        CLIENT_ID | SIGNATURE_KEY_PAIR | MASTER_SECRET | URL_TOKEN | ADB_TOKEN => {
            anyhow::bail!("Rust-only preference")
        }
        _ => None,
    };
    if let Some(command) = command {
        return command.apply(prefs);
    }
    let level = match key {
        LOG_LEVEL => Some(parse_log_level(
            value
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid log level"))?,
        )?),
        DEV_TOKEN => {
            anyhow::ensure!(value.is_string(), "Invalid development token");
            None
        }
        _ => None,
    };
    let changed = prefs.set(key, value)?;
    if let Some(level) = level {
        log::set_max_level(level);
    }
    Ok(changed)
}

pub fn remove_entry(prefs: &Prefs, key: &str) -> Result<bool> {
    let changed = prefs.remove(key)?;
    if key == LOG_LEVEL {
        log::set_max_level(log::LevelFilter::Info);
    }
    Ok(changed)
}

pub fn ensure_signing_keypair(prefs: &Prefs) -> anyhow::Result<Vec<u8>> {
    prefs.update(|map| {
        ensure(map, SIGNATURE_KEY_PAIR, || {
            crate::base64_encode(&crate::ed25519_generate().0)
        })
    })?;
    signing_keypair(prefs)
}

pub fn signing_keypair(prefs: &Prefs) -> anyhow::Result<Vec<u8>> {
    let raw = prefs
        .get::<String>(SIGNATURE_KEY_PAIR)?
        .ok_or_else(|| anyhow::anyhow!("Missing signing identity"))?;
    let bytes = crate::utils::base64::base64_decode_checked(&raw)?;
    anyhow::ensure!(
        crate::ed25519_public_from_keypair(&bytes).is_some(),
        "Invalid signing identity"
    );
    Ok(bytes)
}

fn signature_public_key(prefs: &Prefs) -> anyhow::Result<String> {
    let keypair = signing_keypair(prefs)?;
    Ok(crate::base64_encode(
        &crate::ed25519_public_from_keypair(&keypair).unwrap(),
    ))
}

pub fn get_entry(prefs: &Prefs, key: &str) -> anyhow::Result<Option<Value>> {
    let value = match key {
        PASSWORD | ADB_TOKEN | URL_TOKEN | CLIENT_ID | MDNS_HOSTNAME | SKIP_VERSION => {
            json!(prefs.get::<String>(key)?.unwrap_or_default())
        }
        PASSWORD_TYPE => json!(password_type(prefs)?),
        LOGIN_REQUIRE_APPROVAL => json!(
            prefs
                .get::<bool>(key)?
                .unwrap_or(DEFAULT_LOGIN_REQUIRE_APPROVAL)
        ),
        ROTATE_URL_TOKEN_ON_RESTART => json!(prefs.get::<bool>(key)?.unwrap_or_default()),
        API_PERMISSIONS => json!(prefs.get::<Vec<String>>(key)?.unwrap_or_default()),
        AUTO_CHECK_UPDATE => json!(prefs.get::<bool>(key)?.unwrap_or(true)),
        SIGNATURE_PUBLIC_KEY => json!(signature_public_key(prefs)?),
        _ => return Ok(None),
    };
    Ok(Some(value))
}

pub fn snapshot(prefs: &Prefs) -> anyhow::Result<SecurityState> {
    let map: Map<String, Value> = prefs.entries().into_iter().collect();
    Ok(SecurityState {
        password: read(&map, PASSWORD)?,
        password_type: map
            .get(PASSWORD_TYPE)
            .map(|v| serde_json::from_value(v.clone()))
            .transpose()?
            .unwrap_or(DEFAULT_PASSWORD_TYPE),
        login_require_approval: map
            .get(LOGIN_REQUIRE_APPROVAL)
            .map(|v| serde_json::from_value(v.clone()))
            .transpose()?
            .unwrap_or(DEFAULT_LOGIN_REQUIRE_APPROVAL),
        rotate_url_token_on_restart: read(&map, ROTATE_URL_TOKEN_ON_RESTART)?,
        adb_token: read(&map, ADB_TOKEN)?,
        url_token: read(&map, URL_TOKEN)?,
        api_permissions: read(&map, API_PERMISSIONS)?,
        client_id: read(&map, CLIENT_ID)?,
        mdns_hostname: read(&map, MDNS_HOSTNAME)?,
        signature_public_key: signature_public_key(prefs)?,
    })
}

#[cfg(test)]
#[path = "../../../tests/unit/prefs/system.rs"]
mod tests;

pub const PERMISSIONS: &[&str] = &[
    "WRITE_EXTERNAL_STORAGE",
    "READ_SMS",
    "SEND_SMS",
    "READ_CONTACTS",
    "WRITE_CONTACTS",
    "READ_CALL_LOG",
    "WRITE_CALL_LOG",
    "CALL_PHONE",
    "POST_NOTIFICATIONS",
    "NEARBY_WIFI_DEVICES",
    "ACCESS_FINE_LOCATION",
    "CAMERA",
    "SYSTEM_ALERT_WINDOW",
    "RECORD_AUDIO",
    "READ_MEDIA_IMAGES",
    "READ_MEDIA_VIDEOS",
    "READ_MEDIA_AUDIO",
    "NOTIFICATION_LISTENER",
    "READ_PHONE_STATE",
    "READ_PHONE_NUMBERS",
    "SCHEDULE_EXACT_ALARM",
    "QUERY_ALL_PACKAGES",
    "ADB",
    "CLIPBOARD",
];

pub(crate) fn generate_mdns_hostname() -> String {
    use rand::Rng;
    let alphabet = b"abcdefghjkmnpqrstuwxyz";
    let mut rng = rand::thread_rng();
    let label: String = (0..2)
        .map(|_| alphabet[rng.gen_range(0..alphabet.len())] as char)
        .collect();
    format!("{label}.local")
}

pub fn verify_adb_token(prefs: &Prefs, token: &str) -> Result<bool> {
    let stored = prefs.get::<String>(ADB_TOKEN)?.unwrap_or_default();
    Ok(!token.is_empty() && token == stored)
}
