use super::NativeManagement;
use plain_rs::services::settings::preferences::Preferences;
use plain_rs::services::settings::security_preferences::SecurityPreferences;

use plain_rs::prefs::pref_keys as keys;
use serde_json::{Map, Value, json};

impl NativeManagement {
    pub fn get_prefs(&self, keys: &[String]) -> Result<Map<String, Value>, String> {
        self.state.native_client_id()?;
        keys.iter()
            .map(|key| self.get_pref(key).map(|value| (key.clone(), value)))
            .collect()
    }

    pub fn get_pref(&self, key: &str) -> Result<Value, String> {
        self.state.native_client_id()?;
        if let Some(value) = plain_rs::prefs::settings::get_entry(&self.state.prefs, key)
            .map_err(|error| error.to_string())?
        {
            return Ok(value);
        }
        if let Some(value) = plain_rs::prefs::security::get_entry(&self.state.prefs, key)
            .map_err(|error| error.to_string())?
        {
            return Ok(value);
        }
        let state = &self.state;
        match key {
            keys::UPDATE_INFO => Ok(json!(
                plain_rs::services::settings::updates::info(state)
                    .map_err(|error| error.to_string())?
            )),
            keys::APP_UPDATE_AVAILABLE => {
                let info = plain_rs::services::settings::updates::info(state)
                    .map_err(|error| error.to_string())?;
                let version = &state.host.startup().app_version;
                let skip_version = state
                    .prefs
                    .get::<String>(keys::SKIP_VERSION)
                    .map_err(|error| error.to_string())?
                    .unwrap_or_default();
                Ok(json!(plain_rs::updates::available(
                    &info,
                    version,
                    &skip_version
                )))
            }
            keys::ONBOARDING_COMPLETED => Ok(json!(
                state
                    .store
                    .get()
                    .map_err(|error| error.to_string())?
                    .get_json::<bool>(keys::ONBOARDING_COMPLETED)
                    .map_err(|error| error.to_string())?
                    .unwrap_or(false)
            )),
            _ => Err(format!("Unknown preference: {key}")),
        }
    }

    pub fn preferences(&self) -> Result<Map<String, Value>, String> {
        self.state.native_client_id()?;
        let mut values = plain_rs::prefs::settings::values(&self.state.prefs)
            .map_err(|error| error.to_string())?;
        for key in [
            keys::PASSWORD,
            keys::PASSWORD_TYPE,
            keys::LOGIN_REQUIRE_APPROVAL,
            keys::ROTATE_URL_TOKEN_ON_RESTART,
            keys::ADB_TOKEN,
            keys::URL_TOKEN,
            keys::API_PERMISSIONS,
            keys::CLIENT_ID,
            keys::MDNS_HOSTNAME,
            keys::AUTO_CHECK_UPDATE,
            keys::SKIP_VERSION,
            keys::SIGNATURE_PUBLIC_KEY,
            keys::UPDATE_INFO,
            keys::APP_UPDATE_AVAILABLE,
            keys::ONBOARDING_COMPLETED,
        ] {
            values.insert(key.into(), self.get_pref(key)?);
        }
        Ok(values)
    }

    pub async fn set_pref(&self, key: &str, value: serde_json::Value) -> Result<bool, String> {
        self.state.native_client_id()?;
        plain_rs::services::settings::preferences::Preferences::from_server(&self.state)
            .set(key, value)
            .await
            .map_err(|e| e.to_string())
    }

    fn security_preferences(&self) -> Result<SecurityPreferences<'_>, String> {
        self.state.native_client_id()?;
        Ok(SecurityPreferences {
            prefs: &self.state.prefs,
            events: Some(&self.state.events),
            server: Some(&self.state),
        })
    }

    pub fn reset_password(&self) -> Result<bool, String> {
        self.security_preferences()?
            .reset_password()
            .map_err(|error| error.to_string())?;
        Ok(true)
    }

    pub fn reset_url_token(&self) -> Result<bool, String> {
        self.security_preferences()?
            .reset_url_token()
            .map_err(|error| error.to_string())?;
        Ok(true)
    }

    pub fn reset_adb_token(&self) -> Result<bool, String> {
        self.security_preferences()?
            .reset_adb_token()
            .map_err(|error| error.to_string())?;
        Ok(true)
    }

    pub async fn set_notification_filter_mode(&self, mode: String) -> Result<bool, String> {
        self.state.native_client_id()?;
        Preferences::from_server(&self.state)
            .action(plain_rs::prefs::settings::Action::SetNotificationMode(mode))
            .await
            .map_err(|error| error.to_string())
    }

    pub async fn clear_notification_filter_apps(&self) -> Result<bool, String> {
        self.state.native_client_id()?;
        Preferences::from_server(&self.state)
            .action(plain_rs::prefs::settings::Action::ClearNotificationApps)
            .await
            .map_err(|error| error.to_string())
    }

    pub fn scan_history(&self) -> Result<Vec<String>, String> {
        self.state.native_client_id()?;
        self.state
            .db
            .with_conn(|conn| -> rusqlite::Result<Vec<String>> {
                let mut query = conn.prepare("SELECT text FROM scan_history ORDER BY id DESC")?;
                query.query_map([], |row| row.get(0))?.collect()
            })
            .map_err(|error| error.to_string())
    }

    pub fn add_scan_history(&self, text: String) -> Result<bool, String> {
        self.state.native_client_id()?;
        self.state
            .db
            .with_conn(|conn| conn.execute("INSERT INTO scan_history(text) VALUES (?1)", [&text]))
            .map_err(|error| error.to_string())?;
        Ok(true)
    }

    pub fn remove_scan_history(&self, text: String) -> Result<bool, String> {
        self.state.native_client_id()?;
        self.state
            .db
            .with_conn(|conn| conn.execute("DELETE FROM scan_history WHERE text = ?1", [&text]))
            .map_err(|error| error.to_string())?;
        Ok(true)
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/native/management/preferences.rs"]
mod tests;
