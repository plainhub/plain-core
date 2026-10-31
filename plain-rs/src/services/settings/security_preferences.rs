use plain_rs::prefs::pref_keys::{API_PERMISSIONS, AUTO_CHECK_UPDATE, SKIP_VERSION};
use plain_rs::{
    events::WsEvent,
    prefs::Prefs,
    prefs::security::{self, Command},
};
use serde_json::Value;
use tokio::sync::broadcast;

pub struct SecurityPreferences<'a> {
    pub prefs: &'a Prefs,
    pub events: Option<&'a broadcast::Sender<WsEvent>>,
    pub server: Option<&'a plain_rs::runtime::Runtime>,
}

impl SecurityPreferences<'_> {
    pub async fn set(&self, key: &str, value: Value) -> anyhow::Result<bool> {
        let changed = if matches!(key, AUTO_CHECK_UPDATE | SKIP_VERSION) {
            if let Some(state) = self.server {
                plain_rs::services::settings::updates::set_setting(state, key, value).await?
            } else {
                security::set_entry(self.prefs, key, value)?
            }
        } else {
            security::set_entry(self.prefs, key, value)?
        };
        self.notify(changed);
        if changed && key == API_PERMISSIONS {
            self.notify_permissions_changed().await?;
        }
        if key == plain_rs::prefs::pref_keys::MDNS_HOSTNAME {
            if let Some(state) = self.server {
                state.mdns.update_hostname(state).await?;
            }
        }
        Ok(changed)
    }

    pub async fn remove(&self, key: &str) -> anyhow::Result<bool> {
        let changed = plain_rs::prefs::security::remove_entry(self.prefs, key)?;
        self.notify(changed);
        if changed && key == API_PERMISSIONS {
            self.notify_permissions_changed().await?;
        }
        if key == plain_rs::prefs::pref_keys::MDNS_HOSTNAME {
            if let Some(state) = self.server {
                state.mdns.update_hostname(state).await?;
            }
        }
        Ok(changed)
    }

    pub fn reset_password(&self) -> anyhow::Result<bool> {
        self.apply(Command::ResetPassword)
    }
    pub fn reset_url_token(&self) -> anyhow::Result<bool> {
        self.apply(Command::ResetUrlToken)
    }
    pub fn reset_adb_token(&self) -> anyhow::Result<bool> {
        self.apply(Command::ResetAdbToken)
    }

    fn apply(&self, command: Command) -> anyhow::Result<bool> {
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

    async fn notify_permissions_changed(&self) -> anyhow::Result<()> {
        if let Some(events) = self.events {
            let state = self
                .server
                .ok_or_else(|| anyhow::anyhow!("Permission runtime unavailable"))?;
            let granted = plain_rs::services::device::permissions::granted(&state.host)
                .await
                .map_err(anyhow::Error::msg)?;
            let permissions =
                plain_rs::services::device::permissions::enabled(self.prefs, &granted)
                    .map_err(anyhow::Error::msg)?;
            let _ = events.send(WsEvent::broadcast(
                "PERMISSIONS_UPDATED",
                serde_json::to_string(&permissions)?,
            ));
        }
        Ok(())
    }
}

pub fn security_key(key: &str) -> bool {
    matches!(
        key,
        "password"
            | "password_type"
            | "login_require_approval"
            | "rotate_url_token_on_restart"
            | "api_permissions"
            | "mdns_hostname"
            | "dev_token"
            | "log_level"
            | AUTO_CHECK_UPDATE
            | SKIP_VERSION
    )
}
