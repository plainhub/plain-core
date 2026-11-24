mod command;
use super::pref_keys as keys;
mod password_login;
mod update;
pub use command::Command;
pub use password_login::{
    PasswordCredential, PasswordError, configure_password_login, opaque_public_key,
    password_credential, password_setup_required, set_password_login,
};
pub use update::{ReleaseInfo, UpdateInfo, UpdatePatch};

use super::{Prefs, PrefsError, Result};
use keys::*;
use plain_libs::utils::base64::{base64_decode_checked, base64_encode};
use serde::{Deserialize, Serialize};
use plain_libs::serde_json::{Map, Value, json};

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
    pub adb_token: String,
    pub api_permissions: Vec<crate::features::access::AccessFeature>,
    pub client_id: String,
    pub mdns_hostname: String,
    pub signature_public_key: String,
}

fn read<T: serde::de::DeserializeOwned + Default>(
    map: &Map<String, Value>,
    key: &str,
) -> Result<T> {
    map.get(key)
        .map(|v| plain_libs::serde_json::from_value(v.clone()).map_err(PrefsError::Decode))
        .unwrap_or_else(|| Ok(T::default()))
}
fn put<T: Serialize>(map: &mut Map<String, Value>, key: &str, value: T) -> Result<()> {
    map.insert(
        key.into(),
        plain_libs::serde_json::to_value(value).map_err(PrefsError::Encode)?,
    );
    Ok(())
}
fn ensure(map: &mut Map<String, Value>, key: &str, make: impl FnOnce() -> String) -> Result<()> {
    if read::<String>(map, key)?.is_empty() {
        put(map, key, make())?;
    }
    Ok(())
}

fn decode_private_key(value: &str) -> Result<[u8; 32]> {
    base64_decode_checked(value)
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| PrefsError::Decode(
            <plain_libs::serde_json::Error as serde::de::Error>::custom("Invalid device private key"),
        ))
}

fn ensure_private_key(map: &mut Map<String, Value>, key: &str) -> Result<[u8; 32]> {
    if let Some(value) = map.get(key) {
        let value: String = plain_libs::serde_json::from_value(value.clone()).map_err(PrefsError::Decode)?;
        return decode_private_key(&value);
    }
    let mut secret = [0; 32];
    plain_libs::crypto::gen_random(&mut secret);
    put(map, key, base64_encode(&secret))?;
    Ok(secret)
}
fn random_label(alphabet: &[u8], length: usize) -> String {
    use rand::RngExt;
    let mut rng = rand::rng();
    (0..length)
        .map(|_| alphabet[rng.random_range(0..alphabet.len())] as char)
        .collect()
}

pub fn random_password(length: usize) -> String {
    random_label(
        b"23456789abcdefghijkmnpqrstuvwxyzABCDEFGHJKMNPQRSTUVWXYZ",
        length,
    )
}

pub fn bootstrap(prefs: &Prefs) -> Result<()> {
    password_type(prefs)?;
    prefs.update(|map| {
        ensure(map, CLIENT_ID, crate::features::client_identity::generate)?;
        ensure_private_key(map, SIGNATURE_PRIVATE_KEY)?;
        ensure_private_key(map, ENCRYPTION_PRIVATE_KEY)?;
        map.remove("signature_key_pair");
        ensure(map, MASTER_SECRET, plain_libs::crypto::gen_token)?;
        ensure(map, MDNS_HOSTNAME, generate_mdns_hostname)?;
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

pub fn log_level(prefs: &Prefs) -> plain_libs::anyhow::Result<log::LevelFilter> {
    let value = prefs
        .get::<String>(LOG_LEVEL)?
        .unwrap_or_else(|| "info".into());
    parse_log_level(&value)
}

fn parse_log_level(value: &str) -> plain_libs::anyhow::Result<log::LevelFilter> {
    match value {
        "trace" => Ok(log::LevelFilter::Trace),
        "debug" => Ok(log::LevelFilter::Debug),
        "info" => Ok(log::LevelFilter::Info),
        "warn" => Ok(log::LevelFilter::Warn),
        "error" => Ok(log::LevelFilter::Error),
        _ => plain_libs::anyhow::bail!("Invalid log level"),
    }
}

pub fn set_entry(prefs: &Prefs, key: &str, value: Value) -> plain_libs::anyhow::Result<bool> {
    if key == AUTO_CHECK_UPDATE {
        plain_libs::anyhow::ensure!(value.is_boolean(), "Invalid auto check update setting");
    }
    if key == SKIP_VERSION {
        plain_libs::anyhow::ensure!(value.is_string(), "Invalid skipped version");
    }
    plain_libs::anyhow::ensure!(access(key) != Some(PrefAccess::RustOnly), "Rust-only preference");
    let command = match key {
        PASSWORD => Some(Command::SetPassword {
            password: plain_libs::serde_json::from_value(value.clone())?,
        }),
        PASSWORD_TYPE => Some(Command::SetPasswordType {
            value: plain_libs::serde_json::from_value(value.clone())?,
        }),
        LOGIN_REQUIRE_APPROVAL => Some(Command::SetLoginRequireApproval {
            enabled: plain_libs::serde_json::from_value(value.clone())?,
        }),
        MDNS_HOSTNAME => Some(Command::SetMdnsHostname {
            hostname: plain_libs::serde_json::from_value(value.clone())?,
        }),
        API_PERMISSIONS if value.is_object() => Some(Command::PatchApiPermissions {
            permissions: plain_libs::serde_json::from_value(value.clone())?,
        }),
        API_PERMISSIONS => Some(Command::SetApiPermissions {
            permissions: plain_libs::serde_json::from_value(value.clone())?,
        }),
        _ => None,
    };
    if let Some(command) = command {
        return command.apply(prefs);
    }
    let level = match key {
        LOG_LEVEL => Some(parse_log_level(
            value
                .as_str()
                .ok_or_else(|| plain_libs::anyhow::anyhow!("Invalid log level"))?,
        )?),
        DEV_TOKEN => {
            plain_libs::anyhow::ensure!(value.is_string(), "Invalid development token");
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

pub fn ensure_signing_keypair(prefs: &Prefs) -> plain_libs::anyhow::Result<Vec<u8>> {
    let seed = prefs.update(|map| ensure_private_key(map, SIGNATURE_PRIVATE_KEY))?;
    Ok(plain_libs::crypto::ed25519_keypair_from_seed(&seed).to_vec())
}

pub fn signing_keypair(prefs: &Prefs) -> plain_libs::anyhow::Result<Vec<u8>> {
    let seed = private_key(prefs, SIGNATURE_PRIVATE_KEY)?;
    Ok(plain_libs::crypto::ed25519_keypair_from_seed(&seed).to_vec())
}

fn private_key(prefs: &Prefs, key: &str) -> plain_libs::anyhow::Result<[u8; 32]> {
    let raw = prefs.get::<String>(key)?
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Missing device private key"))?;
    Ok(decode_private_key(&raw)?)
}

pub(crate) fn encryption_private_key(prefs: &Prefs) -> plain_libs::anyhow::Result<[u8; 32]> {
    private_key(prefs, ENCRYPTION_PRIVATE_KEY)
}

pub fn encryption_public_key(prefs: &Prefs) -> plain_libs::anyhow::Result<[u8; 32]> {
    Ok(plain_libs::crypto::hpke::public_key(&encryption_private_key(prefs)?))
}

fn signature_public_key(prefs: &Prefs) -> plain_libs::anyhow::Result<String> {
    let keypair = signing_keypair(prefs)?;
    Ok(base64_encode(&keypair[32..]))
}

pub fn get_entry(prefs: &Prefs, key: &str) -> plain_libs::anyhow::Result<Option<Value>> {
    let value = match key {
        PASSWORD => json!(""),
        ADB_TOKEN | CLIENT_ID | MDNS_HOSTNAME | SKIP_VERSION => {
            json!(prefs.get::<String>(key)?.unwrap_or_default())
        }
        PASSWORD_TYPE => json!(password_type(prefs)?),
        LOGIN_REQUIRE_APPROVAL => json!(
            prefs
                .get::<bool>(key)?
                .unwrap_or(DEFAULT_LOGIN_REQUIRE_APPROVAL)
        ),
        API_PERMISSIONS => json!(
            prefs
                .get::<Vec<crate::features::access::AccessFeature>>(key)?
                .unwrap_or_default()
        ),
        AUTO_CHECK_UPDATE => json!(prefs.get::<bool>(key)?.unwrap_or(true)),
        SIGNATURE_PUBLIC_KEY => json!(signature_public_key(prefs)?),
        _ => return Ok(None),
    };
    Ok(Some(value))
}

pub fn snapshot(prefs: &Prefs) -> plain_libs::anyhow::Result<SecurityState> {
    let map: Map<String, Value> = prefs.entries().into_iter().collect();
    Ok(SecurityState {
        password: String::new(),
        password_type: map
            .get(PASSWORD_TYPE)
            .map(|v| plain_libs::serde_json::from_value(v.clone()))
            .transpose()?
            .unwrap_or(DEFAULT_PASSWORD_TYPE),
        login_require_approval: map
            .get(LOGIN_REQUIRE_APPROVAL)
            .map(|v| plain_libs::serde_json::from_value(v.clone()))
            .transpose()?
            .unwrap_or(DEFAULT_LOGIN_REQUIRE_APPROVAL),
        adb_token: read(&map, ADB_TOKEN)?,
        api_permissions: read(&map, API_PERMISSIONS)?,
        client_id: read(&map, CLIENT_ID)?,
        mdns_hostname: read(&map, MDNS_HOSTNAME)?,
        signature_public_key: signature_public_key(prefs)?,
    })
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/prefs/system.rs"]
mod tests;

pub(crate) fn generate_mdns_hostname() -> String {
    let label = random_label(b"abcdefghjkmnpqrstuwxyz", 2);
    format!("{label}.local")
}

pub fn verify_adb_token(prefs: &Prefs, token: &str) -> Result<bool> {
    let stored = prefs.get::<String>(ADB_TOKEN)?.unwrap_or_default();
    Ok(!token.is_empty() && token == stored)
}
