use crate::features::prefs::error::PreferenceErrorCode;
use crate::features::enums::DataType;
use crate::features::prefs::Prefs;
use crate::features::prefs::pref_keys::{
    AI_IMAGE_SEARCH_MODEL, MEDIA_EXCLUDED_DIRS, RECENT_SELECTED_DIRS,
};
use crate::infrastructure::events::WsEvent;
use plain_libs::serde_json::Value;
use tokio::sync::broadcast;

pub struct Preferences<'a> {
    pub prefs: &'a Prefs,
    pub events: Option<&'a broadcast::Sender<WsEvent>>,
    pub server: Option<&'a crate::runtime::Runtime>,
    pub client_id: Option<&'a str>,
}

impl<'a> Preferences<'a> {
    pub fn from_server(state: &'a crate::runtime::Runtime) -> Self {
        Self {
            prefs: &state.prefs,
            events: Some(&state.events),
            server: Some(state),
            client_id: None,
        }
    }

    pub async fn patch(
        &self,
        patch: crate::features::prefs::settings::SettingsPatch,
    ) -> plain_libs::anyhow::Result<bool> {
        let device_name = patch.device_name.is_some();
        self.update(device_name, move |prefs| {
            crate::features::prefs::settings::patch(prefs, patch)
        })
        .await
    }

    pub async fn set(&self, key: &str, value: Value) -> plain_libs::anyhow::Result<bool> {
        plain_libs::anyhow::ensure!(
            crate::features::prefs::pref_keys::writable(key, true, &[]),
            "Preference is not accessible"
        );
        plain_libs::anyhow::ensure!(value.to_string().len() <= 65536, PreferenceErrorCode::InvalidPrefValue);
        if key == MEDIA_EXCLUDED_DIRS {
            crate::features::media::visibility::validate(&value)?;
        }
        if key == RECENT_SELECTED_DIRS {
            let dirs: Vec<String> = plain_libs::serde_json::from_value(value.clone())?;
            plain_libs::anyhow::ensure!(
                dirs.len() <= 8 && dirs.iter().all(|dir| {
                    !dir.contains('\0') && (dir.starts_with('/') ||
                        matches!(dir.as_bytes(), [drive, b':', b'/', ..] if drive.is_ascii_alphabetic()))
                }),
                "Expected at most 8 absolute directories"
            );
        }
        if key == AI_IMAGE_SEARCH_MODEL {
            plain_libs::anyhow::ensure!(
                matches!(value.as_str(), Some("default" | "imported")),
                "Unknown image search model"
            );
        }
        if crate::features::services::settings::security_preferences::security_key(key) {
            return crate::features::services::settings::security_preferences::SecurityPreferences {
                prefs: self.prefs,
                events: self.events,
                server: self.server,
            }
            .set(key, value)
            .await;
        }
        self.update(
            key == crate::features::prefs::pref_keys::DEVICE_NAME,
            move |prefs| match crate::features::prefs::settings::set_entry(
                prefs,
                key,
                value.clone(),
            )? {
                Some(changed) => Ok(changed),
                None => Ok(prefs.set(key, value)?),
            },
        )
        .await
    }

    pub async fn remove(&self, key: &str) -> plain_libs::anyhow::Result<bool> {
        self.update(
            key == crate::features::prefs::pref_keys::DEVICE_NAME,
            move |prefs| Ok(prefs.remove(key)?),
        )
        .await
    }

    pub async fn action(
        &self,
        action: crate::features::prefs::settings::Action,
    ) -> plain_libs::anyhow::Result<bool> {
        self.update(false, move |prefs| action.apply(prefs)).await
    }

    async fn update(
        &self,
        device_name: bool,
        write: impl FnOnce(&Prefs) -> plain_libs::anyhow::Result<bool>,
    ) -> plain_libs::anyhow::Result<bool> {
        let before = crate::features::prefs::settings::snapshot(self.prefs)?;
        let before_media_dirs = self
            .prefs
            .get::<Vec<String>>(MEDIA_EXCLUDED_DIRS)?
            .unwrap_or_default();
        let before_model = self
            .prefs
            .get_or(AI_IMAGE_SEARCH_MODEL, String::from("default"));
        let actor = match self.server.filter(|_| device_name) {
            Some(state) => {
                let id = match self.client_id {
                    Some(id) => id.to_owned(),
                    None => state
                        .native_client_id()
                        .map_err(plain_libs::anyhow::Error::msg)?,
                };
                Some(super::super::audit_events::client(state, &id)?)
            }
            None => None,
        };
        let changed = write(self.prefs)?;
        let after = crate::features::prefs::settings::snapshot(self.prefs)?;
        if before.device_name != after.device_name {
            if let Some(state) = self.server {
                state
                    .audit_events
                    .record(
                        crate::features::audit_events::AuditEvent::UpdateDeviceName(
                            crate::features::audit_events::DeviceNameEventData {
                                old_name: before.device_name.clone(),
                                new_name: after.device_name.clone(),
                            },
                        ),
                        actor,
                    )
                    .map_err(plain_libs::anyhow::Error::msg)?;
            }
        }
        if changed {
            if let Some(events) = self.events {
                let _ = events.send(WsEvent::broadcast("PREFS_UPDATED", "{}".into()));
                if before_media_dirs
                    != self
                        .prefs
                        .get::<Vec<String>>(MEDIA_EXCLUDED_DIRS)?
                        .unwrap_or_default()
                {
                    crate::features::services::files::catalog_events::publish(
                        events,
                        [
                            DataType::Image,
                            DataType::Video,
                            DataType::Audio,
                            DataType::Doc,
                        ]
                        .map(DataType::kind),
                    );
                }
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
                    .map_err(plain_libs::anyhow::Error::msg)?;
            }
            if before.remote_access && !after.remote_access {
                state.clients.close(None);
            }
            if before.dlna != after.dlna {
                crate::features::services::casting::receiver::lifecycle::reconcile(state)
                    .await
                    .map_err(plain_libs::anyhow::Error::msg)?;
            }
            if before.device_name != after.device_name
                || before.remote_access != after.remote_access
            {
                state.mdns.update_hostname(state).await?;
            }
        }
        if let Some(events) = self.events {
            if before.device_name != after.device_name {
                let _ = events.send(WsEvent::broadcast(
                    "DEVICE_NAME_UPDATED",
                    plain_libs::serde_json::to_string(&after.device_name)?,
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
#[path = "../../../../../tests/plain-rs/unit/features/services/settings/preferences.rs"]
mod tests;
