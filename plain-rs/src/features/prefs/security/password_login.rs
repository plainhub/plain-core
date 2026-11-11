use super::*;

#[derive(Debug)]
pub enum PasswordError {
    InvalidPassword,
    Preferences(PrefsError),
}

impl std::fmt::Display for PasswordError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPassword => formatter.write_str("Password must contain 1 to 1024 bytes"),
            Self::Preferences(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for PasswordError {}

fn validate_password(password: &str) -> std::result::Result<(), PasswordError> {
    if password.trim().is_empty() || password.len() > 1024 {
        return Err(PasswordError::InvalidPassword);
    }
    Ok(())
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PasswordCredential {
    pub setup: String,
    pub record: String,
}

impl std::fmt::Debug for PasswordCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PasswordCredential([redacted])")
    }
}

pub(super) fn write_password(map: &mut Map<String, Value>, password: &str) -> Result<()> {
    ensure(map, SIGNATURE_KEY_PAIR, || {
        plain_libs::utils::base64::base64_encode(&plain_libs::crypto::ed25519_generate().0)
    })?;
    let pair = plain_libs::utils::base64::base64_decode(&read::<String>(map, SIGNATURE_KEY_PAIR)?);
    if pair.len() != 64 {
        return Err(PrefsError::Decode(
            <serde_json::Error as serde::de::Error>::custom("Invalid signing identity"),
        ));
    }
    let registration = plain_libs::auth::server::register(password.as_bytes(), &pair[32..])
        .map_err(|_| {
            PrefsError::Encode(<serde_json::Error as serde::ser::Error>::custom(
                "Password registration failed",
            ))
        })?;
    put(
        map,
        OPAQUE_SERVER_SETUP,
        plain_libs::utils::base64::base64_encode(&registration.setup),
    )?;
    put(
        map,
        OPAQUE_PASSWORD_RECORD,
        plain_libs::utils::base64::base64_encode(&registration.record),
    )
}

fn write_password_login(map: &mut Map<String, Value>, password: &str) -> Result<()> {
    write_password(map, password)?;
    put(map, PASSWORD_TYPE, PasswordType::Fixed)?;
    put(map, LOGIN_REQUIRE_APPROVAL, false)
}

pub fn configure_password_login(prefs: &Prefs) -> Result<()> {
    let needs_setup = password_type(prefs)? == PasswordType::None;
    prefs.update(|map| {
        if needs_setup {
            put(map, PASSWORD_TYPE, PasswordType::Fixed)?;
        }
        put(map, LOGIN_REQUIRE_APPROVAL, false)
    })
}

pub fn password_setup_required(prefs: &Prefs) -> Result<bool> {
    Ok(password_type(prefs)? == PasswordType::Fixed && password_credential(prefs)?.is_none())
}

pub fn password_credential(prefs: &Prefs) -> Result<Option<PasswordCredential>> {
    let map: Map<String, Value> = prefs.entries().into_iter().collect();
    let setup: Option<String> = map
        .get(OPAQUE_SERVER_SETUP)
        .map(|v| serde_json::from_value(v.clone()).map_err(PrefsError::Decode))
        .transpose()?;
    let record: Option<String> = map
        .get(OPAQUE_PASSWORD_RECORD)
        .map(|v| serde_json::from_value(v.clone()).map_err(PrefsError::Decode))
        .transpose()?;
    let invalid = || {
        PrefsError::Decode(<serde_json::Error as serde::de::Error>::custom(
            "Invalid password registration",
        ))
    };
    match (setup, record) {
        (None, None) => Ok(None),
        (Some(setup), Some(record)) => {
            let setup_bytes =
                plain_libs::utils::base64::base64_decode_checked(&setup).map_err(|_| invalid())?;
            let record_bytes =
                plain_libs::utils::base64::base64_decode_checked(&record).map_err(|_| invalid())?;
            plain_libs::auth::server::validate_registration(&setup_bytes, &record_bytes)
                .map_err(|_| invalid())?;
            Ok(Some(PasswordCredential { setup, record }))
        }
        _ => Err(invalid()),
    }
}

pub fn opaque_public_key(prefs: &Prefs) -> Result<Option<String>> {
    password_credential(prefs)?
        .map(|credential| {
            plain_libs::auth::server::public_key(&plain_libs::utils::base64::base64_decode(
                &credential.setup,
            ))
            .map(|key| plain_libs::utils::base64::base64_encode(&key))
            .map_err(|_| {
                PrefsError::Decode(<serde_json::Error as serde::de::Error>::custom(
                    "Invalid password registration",
                ))
            })
        })
        .transpose()
}

pub fn set_password_login(prefs: &Prefs, password: &str) -> std::result::Result<(), PasswordError> {
    validate_password(password)?;
    prefs
        .update(|map| write_password_login(map, password))
        .map_err(PasswordError::Preferences)
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/prefs/security/password_login.rs"]
mod tests;
