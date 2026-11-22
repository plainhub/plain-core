use crate::http::auth::public_gate::LocalOwner;
use crate::server::ServerState;
use plain_graphql::{Context, Enum, Object, Result};
use plain_rs::features::legacy_upgrade::Status;
use plain_rs::infrastructure::events::{LEGACY_DATA_UPGRADE_CHANGED, WsEvent};

pub(crate) fn snapshot(state: &ServerState) -> Result<Status> {
    if let Some(storage) = &state.host.startup().legacy_storage {
        Ok(storage.status(&state.db, &state.directory)?)
    } else {
        Ok(Status::Unavailable)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Enum)]
pub enum LegacyDataUpgradeAction {
    Skip,
    Import,
    Clear,
}

#[derive(Default)]
pub struct LegacyDataMutation;

#[Object]
impl LegacyDataMutation {
    /// Skip the startup prompt, import old data, or clear the imported source data on this device.
    #[graphql(guard = "LocalOwner")]
    async fn update_legacy_data_upgrade(
        &self,
        ctx: &Context<'_>,
        action: LegacyDataUpgradeAction,
    ) -> Result<Status> {
        let state = ctx.data::<ServerState>()?.clone();
        let storage = state
            .host
            .startup()
            .legacy_storage
            .clone()
            .ok_or("Legacy data is unavailable")?;
        let result = tokio::task::spawn_blocking(move || {
            let status = match action {
                LegacyDataUpgradeAction::Skip => storage.skip(&state.db, &state.directory)?,
                LegacyDataUpgradeAction::Import => {
                    storage.import(&state.db, &state.prefs, &state.directory)?
                }
                LegacyDataUpgradeAction::Clear => storage.clear(&state.db, &state.directory)?,
            };
            if action != LegacyDataUpgradeAction::Clear {
                let _ = state
                    .events
                    .send(WsEvent::broadcast("PREFS_UPDATED", "{}".into()));
            }
            if action == LegacyDataUpgradeAction::Import && status == Status::Imported {
                let _ = state
                    .events
                    .send(WsEvent::broadcast("CONTENT_CHANGED", "{}".into()));
                let _ = state
                    .events
                    .send(WsEvent::broadcast("PEER_UPDATED", "{}".into()));
                let _ = state
                    .events
                    .send(WsEvent::broadcast("POMODORO_SETTINGS_UPDATE", "{}".into()));
            }
            let _ = state
                .events
                .send(WsEvent::broadcast(LEGACY_DATA_UPGRADE_CHANGED, "{}".into()));
            Ok::<_, plain_libs::anyhow::Error>(status)
        })
        .await?;
        Ok(result?)
    }
}
