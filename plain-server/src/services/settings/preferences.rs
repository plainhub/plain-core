use plain_rs::prefs::pref_keys::AI_IMAGE_SEARCH_MODEL;
use plain_rs::{events::WsEvent, prefs::Prefs};
use serde_json::Value;
use tokio::sync::broadcast;

pub(crate) struct Preferences<'a> {
    pub(crate) prefs: &'a Prefs,
    pub(crate) events: Option<&'a broadcast::Sender<WsEvent>>,
    pub(crate) server: Option<&'a crate::assembly::server::ServerState>,
}

impl<'a> Preferences<'a> {
    pub(crate) fn from_server(state: &'a crate::assembly::server::ServerState) -> Self {
        Self {
            prefs: &state.prefs,
            events: Some(&state.events),
            server: Some(state),
        }
    }

    pub(crate) async fn patch(
        &self,
        patch: plain_rs::prefs::settings::SettingsPatch,
    ) -> anyhow::Result<bool> {
        self.update(move |prefs| plain_rs::prefs::settings::patch(prefs, patch))
            .await
    }

    pub(crate) async fn set(&self, key: &str, value: Value) -> anyhow::Result<bool> {
        anyhow::ensure!(
            plain_rs::prefs::pref_keys::writable(key, true),
            "Preference is not accessible"
        );
        anyhow::ensure!(value.to_string().len() <= 65536, "invalid_pref_value");
        if key == AI_IMAGE_SEARCH_MODEL {
            anyhow::ensure!(
                matches!(value.as_str(), Some("default" | "imported")),
                "Unknown image search model"
            );
        }
        if super::security_preferences::security_key(key) {
            return super::security_preferences::SecurityPreferences {
                prefs: self.prefs,
                events: self.events,
                server: self.server,
            }
            .set(key, value)
            .await;
        }
        self.update(move |prefs| {
            match plain_rs::prefs::settings::set_entry(prefs, key, value.clone())? {
                Some(changed) => Ok(changed),
                None => Ok(prefs.set(key, value)?),
            }
        })
        .await
    }

    pub(crate) async fn remove(&self, key: &str) -> anyhow::Result<bool> {
        self.update(move |prefs| Ok(prefs.remove(key)?)).await
    }

    pub(crate) async fn action(
        &self,
        action: plain_rs::prefs::settings::Action,
    ) -> anyhow::Result<bool> {
        self.update(move |prefs| action.apply(prefs)).await
    }

    async fn update(
        &self,
        write: impl FnOnce(&Prefs) -> anyhow::Result<bool>,
    ) -> anyhow::Result<bool> {
        let before = plain_rs::prefs::settings::snapshot(self.prefs)?;
        let before_model = self
            .prefs
            .get_or(AI_IMAGE_SEARCH_MODEL, String::from("default"));
        let changed = write(self.prefs)?;
        let after = plain_rs::prefs::settings::snapshot(self.prefs)?;
        if changed {
            if let Some(events) = self.events {
                let _ = events.send(WsEvent::broadcast("PREFS_UPDATED", "{}".into()));
            }
        }
        if let Some(state) = self.server {
            if after.ai_image_search_enabled
                && before_model
                    != self
                        .prefs
                        .get_or(AI_IMAGE_SEARCH_MODEL, String::from("default"))
            {
                state.image_models.shutdown().await;
                state
                    .image_models
                    .enable(false)
                    .await
                    .map_err(anyhow::Error::msg)?;
            }
            if before.desktop_access && !after.desktop_access {
                state.main_ws.close_client(state, None);
            }
            if before.dlna != after.dlna {
                crate::services::casting::receiver::lifecycle::reconcile(state)
                    .await
                    .map_err(anyhow::Error::msg)?;
            }
            if before.device_name != after.device_name {
                state.mdns.update_hostname(state).await?;
            }
        }
        if let Some(events) = self.events {
            if before.device_name != after.device_name {
                let _ = events.send(WsEvent::broadcast(
                    "DEVICE_NAME_UPDATED",
                    serde_json::to_string(&after.device_name)?,
                ));
            }
            if before.notification_filter != after.notification_filter {
                let _ = events.send(WsEvent::broadcast("NOTIFICATION_REFRESHED", String::new()));
            }
            if before.pomodoro_settings != after.pomodoro_settings {
                let _ = events.send(WsEvent::broadcast(
                    "POMODORO_SETTINGS_UPDATE",
                    after.pomodoro_settings.to_string(),
                ));
            }
        }
        Ok(changed)
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/services/settings/preferences.rs"]
mod tests;
