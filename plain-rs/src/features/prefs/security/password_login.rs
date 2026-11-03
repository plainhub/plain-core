use super::*;

#[derive(Debug)]
pub enum PasswordSetupError {
    InvalidPassword,
    Preferences(PrefsError),
}

impl std::fmt::Display for PasswordSetupError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPassword => formatter.write_str("Password must contain 1 to 1024 bytes"),
            Self::Preferences(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for PasswordSetupError {}

fn validate_password(password: &str) -> std::result::Result<(), PasswordSetupError> {
    if password.trim().is_empty() || password.len() > 1024 {
        return Err(PasswordSetupError::InvalidPassword);
    }
    Ok(())
}

pub(super) fn write_password(map: &mut Map<String, Value>, password: &str) -> Result<()> {
    put(
        map,
        PASSWORD,
        plain_libs::crypto::password::PasswordHash::new(
            plain_libs::crypto::sha512_hex(password).as_bytes(),
        ),
    )
}

fn write_password_login(map: &mut Map<String, Value>, password: &str) -> Result<()> {
    write_password(map, password)?;
    put(map, PASSWORD_TYPE, PasswordType::Fixed)?;
    put(map, LOGIN_REQUIRE_APPROVAL, false)
}

pub fn configure_password_login(prefs: &Prefs) -> Result<()> {
    if password_type(prefs)? == PasswordType::None {
        prefs.update(|map| {
            map.remove(PASSWORD);
            put(map, PASSWORD_TYPE, PasswordType::Fixed)?;
            put(map, LOGIN_REQUIRE_APPROVAL, false)
        })?;
    }
    Ok(())
}

pub fn password_setup_required(prefs: &Prefs) -> Result<bool> {
    Ok(password_type(prefs)? == PasswordType::Fixed && password_hash(prefs)?.is_none())
}

pub fn password_hash(prefs: &Prefs) -> Result<Option<plain_libs::crypto::password::PasswordHash>> {
    prefs.get(PASSWORD)
}

pub fn valid_password_digest(digest: &str) -> bool {
    digest.len() == 128
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub fn setup_password(
    prefs: &Prefs,
    password: &str,
) -> std::result::Result<bool, PasswordSetupError> {
    validate_password(password)?;
    setup_password_hash(prefs, &plain_libs::crypto::sha512_hex(password))
}

pub fn setup_password_hash(
    prefs: &Prefs,
    digest: &str,
) -> std::result::Result<bool, PasswordSetupError> {
    if !valid_password_digest(digest) {
        return Err(PasswordSetupError::InvalidPassword);
    }
    prefs
        .update(|map| {
            if map.get(PASSWORD_TYPE) != Some(&json!(PasswordType::Fixed))
                || map.contains_key(PASSWORD)
            {
                return Ok(false);
            }
            put(
                map,
                PASSWORD,
                plain_libs::crypto::password::PasswordHash::new(digest.as_bytes()),
            )?;
            put(map, LOGIN_REQUIRE_APPROVAL, false)?;
            Ok(true)
        })
        .map_err(PasswordSetupError::Preferences)
}

pub fn set_password_login(
    prefs: &Prefs,
    password: &str,
) -> std::result::Result<(), PasswordSetupError> {
    validate_password(password)?;
    prefs
        .update(|map| write_password_login(map, password))
        .map_err(PasswordSetupError::Preferences)
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/prefs/security/password_login.rs"]
mod tests;
