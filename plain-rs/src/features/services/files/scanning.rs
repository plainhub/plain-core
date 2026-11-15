use crate::runtime::Runtime as CoreRuntime;
use plain_libs::anyhow::Result;

pub struct Progress {
    pub volume_id: Option<String>,
    pub indexed: i32,
    pub total: Option<i32>,
    pub state: FileIndexScanState,
    pub error: Option<String>,
}
/// File-index scan lifecycle shared by query and subscription progress.
#[derive(plain_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FileIndexScanState {
    Idle,
    Queued,
    Running,
    Paused,
    Stopped,
    Failed,
}
pub enum Command {
    Rebuild(Option<String>),
    Pause(Option<String>),
    Resume(Option<String>),
    Stop(Option<String>),
}

async fn available(state: &CoreRuntime) -> Result<()> {
    crate::features::services::capabilities::require(&state.host, "MEDIA_SCAN").await
}
pub async fn progress(state: &CoreRuntime, volume_id: Option<&str>) -> Result<Progress> {
    available(state).await?;
    #[cfg(feature = "runtime-media")]
    {
        use crate::features::services::files::media_scans::Status;
        let snapshot = match volume_id {
            Some(id) => state.media_scans.snapshot_for(id),
            None => state.media_scans.snapshot(),
        }
        .map_err(plain_libs::anyhow::Error::msg)?;
        let Some(snapshot) = snapshot else {
            return Ok(Progress {
                volume_id: volume_id.map(str::to_owned),
                indexed: 0,
                total: Some(0),
                state: FileIndexScanState::Idle,
                error: None,
            });
        };
        Ok(Progress {
            volume_id: snapshot.volume_id,
            indexed: snapshot.processed_files.try_into()?,
            total: snapshot.total_files.map(i32::try_from).transpose()?,
            state: match snapshot.status {
                Status::Queued => FileIndexScanState::Queued,
                Status::Running => FileIndexScanState::Running,
                Status::Paused => FileIndexScanState::Paused,
                Status::Canceling | Status::Canceled => FileIndexScanState::Stopped,
                Status::Completed => FileIndexScanState::Idle,
                Status::Failed => FileIndexScanState::Failed,
            },
            error: snapshot.error,
        })
    }
    #[cfg(not(feature = "runtime-media"))]
    plain_libs::anyhow::bail!("MEDIA_SCAN advertised without a scanner implementation")
}
pub async fn execute(state: &CoreRuntime, command: Command) -> Result<()> {
    available(state).await?;
    #[cfg(feature = "runtime-media")]
    {
        let catalog =
            crate::features::services::files::catalog_scan::NativeMediaCatalog::new(state.clone());
        if matches!(&command, Command::Rebuild(_)) {
            let volumes = crate::features::services::files::indexing::discover(state)
                .await
                .map_err(plain_libs::anyhow::Error::msg)?;
            let policy = state
                .media_scans
                .policy()
                .map_err(plain_libs::anyhow::Error::msg)?;
            if policy
                .scope
                .as_ref()
                .is_none_or(|scope| !scope.matches_volumes(&volumes))
            {
                catalog
                    .configure_policy((*policy).clone())
                    .map_err(plain_libs::anyhow::Error::msg)?;
                catalog
                    .watch_source_directories()
                    .map_err(plain_libs::anyhow::Error::msg)?;
            }
        }
        match command {
            Command::Rebuild(volume_id) => {
                catalog
                    .start_volume_scan(volume_id.as_deref())
                    .map_err(plain_libs::anyhow::Error::msg)?
                    .ok_or_else(|| {
                        plain_libs::anyhow::anyhow!(
                            "Volume is offline or has no configured index directories"
                        )
                    })?;
            }
            Command::Pause(volume_id) => {
                state
                    .media_scans
                    .pause(volume_id.as_deref())
                    .map_err(plain_libs::anyhow::Error::msg)?;
            }
            Command::Resume(volume_id) => {
                state
                    .media_scans
                    .resume(volume_id.as_deref())
                    .map_err(plain_libs::anyhow::Error::msg)?;
            }
            Command::Stop(volume_id) => {
                state
                    .media_scans
                    .cancel(volume_id.as_deref())
                    .map_err(plain_libs::anyhow::Error::msg)?;
            }
        }
        Ok(())
    }
    #[cfg(not(feature = "runtime-media"))]
    {
        let _ = command;
        plain_libs::anyhow::bail!("MEDIA_SCAN advertised without a scanner implementation")
    }
}
