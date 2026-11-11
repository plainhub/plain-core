use crate::features::prefs::Prefs;
use crate::features::prefs::pref_keys::{API_PERMISSIONS, AUTO_CHECK_UPDATE, SKIP_VERSION};
use crate::features::prefs::security;
use crate::features::prefs::security::Command;
use crate::infrastructure::events::WsEvent;
use serde_json::Value;
use tokio::sync::broadcast;

pub struct SecurityPreferences<'a> {
    pub prefs: &'a Prefs,
    pub events: Option<&'a broadcast::Sender<WsEvent>>,
    pub server: Option<&'a crate::runtime::Runtime>,
}

impl SecurityPreferences<'_> {
    pub async fn set(&self, key: &str, value: Value) -> plain_libs::anyhow::Result<bool> {
        let changed = if matches!(key, AUTO_CHECK_UPDATE | SKIP_VERSION) {
            if let Some(state) = self.server {
                crate::features::services::settings::updates::set_setting(state, key, value).await?
            } else {
                security::set_entry(self.prefs, key, value)?
            }
        } else {
            security::set_entry(self.prefs, key, value)?
        };
        self.notify(changed);
        if changed && key == API_PERMISSIONS {
            self.notify_access_changed();
        }
        if key == crate::features::prefs::pref_keys::MDNS_HOSTNAME {
            if let Some(state) = self.server {
                state.mdns.update_hostname(state).await?;
            }
        }
        Ok(changed)
    }

    pub async fn remove(&self, key: &str) -> plain_libs::anyhow::Result<bool> {
        let changed = crate::features::prefs::security::remove_entry(self.prefs, key)?;
        self.notify(changed);
        if changed && key == API_PERMISSIONS {
            self.notify_access_changed();
        }
        if key == crate::features::prefs::pref_keys::MDNS_HOSTNAME {
            if let Some(state) = self.server {
                state.mdns.update_hostname(state).await?;
            }
        }
        Ok(changed)
    }

    pub fn generate_password(&self) -> plain_libs::anyhow::Result<String> {
        Ok(security::random_password(6))
    }
    pub fn reset_adb_token(&self) -> plain_libs::anyhow::Result<bool> {
        self.apply(Command::ResetAdbToken)
    }

    fn apply(&self, command: Command) -> plain_libs::anyhow::Result<bool> {
        let changed = command.apply(self.prefs)?;
        self.notify(changed);
        Ok(changed)
    }

    fn notify(&self, changed: bool) {
        if changed {
            if let Some(events) = self.events {
                let _ = events.send(WsEvent::broadcast("PREFS_UPDATED", "{}".into()));
            }
        }
    }

    fn notify_access_changed(&self) {
        if let Some(events) = self.events {
            let _ = events.send(WsEvent::broadcast("ACCESS_UPDATED", "true".into()));
        }
    }

}

pub fn security_key(key: &str) -> bool {
    matches!(
        key,
        "password"
            | "password_type"
            | "login_require_approval"
            | "api_permissions"
            | "mdns_hostname"
            | "dev_token"
            | "log_level"
            | AUTO_CHECK_UPDATE
            | SKIP_VERSION
    )
}
