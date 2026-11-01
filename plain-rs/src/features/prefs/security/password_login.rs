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

fn write_password_login(map: &mut Map<String, Value>, password: &str) -> Result<()> {
    put(map, PASSWORD, password)?;
    put(map, PASSWORD_TYPE, PasswordType::Fixed)?;
    put(map, LOGIN_REQUIRE_APPROVAL, false)
}

pub fn configure_password_login(prefs: &Prefs) -> Result<()> {
    if password_type(prefs)? == PasswordType::None {
        prefs.update(|map| write_password_login(map, ""))?;
    }
    Ok(())
}

pub fn password_setup_required(prefs: &Prefs) -> Result<bool> {
    Ok(password_type(prefs)? == PasswordType::Fixed
        && prefs
            .get::<String>(PASSWORD)?
            .unwrap_or_default()
            .is_empty())
}

pub fn setup_password(
    prefs: &Prefs,
    password: &str,
) -> std::result::Result<bool, PasswordSetupError> {
    validate_password(password)?;
    prefs
        .update(|map| {
            if map.get(PASSWORD_TYPE) != Some(&json!(PasswordType::Fixed))
                || !read::<String>(map, PASSWORD)?.is_empty()
            {
                return Ok(false);
            }
            write_password_login(map, password)?;
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
#[path = "../../../../tests/unit/features/prefs/security/password_login.rs"]
mod tests;
