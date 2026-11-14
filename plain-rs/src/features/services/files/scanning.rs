use crate::runtime::Runtime as CoreRuntime;
use plain_libs::anyhow::Result;

pub struct Progress {
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
    Running,
    Paused,
    Stopped,
    Failed,
}
pub enum Command {
    Rebuild(Option<String>),
    Pause,
    Resume,
    Stop,
}

async fn available(state: &CoreRuntime) -> Result<()> {
    crate::features::services::capabilities::require(&state.host, "MEDIA_SCAN").await
}
pub async fn progress(state: &CoreRuntime) -> Result<Progress> {
    available(state).await?;
    #[cfg(feature = "runtime-media")]
    {
        use crate::features::services::files::media_scans::Status;
        let Some(snapshot) = state
            .media_scans
            .snapshot()
            .map_err(plain_libs::anyhow::Error::msg)?
        else {
            return Ok(Progress {
                indexed: 0,
                total: Some(0),
                state: FileIndexScanState::Idle,
                error: None,
            });
        };
        Ok(Progress {
            indexed: snapshot.processed_files.try_into()?,
            total: snapshot.total_files.map(i32::try_from).transpose()?,
            state: match snapshot.status {
                Status::Running => FileIndexScanState::Running,
                Status::Paused => FileIndexScanState::Paused,
                Status::Cancelling | Status::Cancelled => FileIndexScanState::Stopped,
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
            crate::features::services::files::indexing::discover(state)
                .await
                .map_err(plain_libs::anyhow::Error::msg)?;
            catalog
                .configure_policy(
                    (*state
                        .media_scans
                        .policy()
                        .map_err(plain_libs::anyhow::Error::msg)?)
                    .clone(),
                )
                .map_err(plain_libs::anyhow::Error::msg)?;
            catalog
                .watch_source_directories()
                .map_err(plain_libs::anyhow::Error::msg)?;
        }
        match command {
            Command::Rebuild(volume_id) => match volume_id {
                Some(id) => {
                    catalog
                        .start_volume_scan(&id)
                        .map_err(plain_libs::anyhow::Error::msg)?
                        .ok_or_else(|| {
                            plain_libs::anyhow::anyhow!(
                                "Volume is offline or has no configured index directories"
                            )
                        })?;
                }
                None => {
                    catalog
                        .start_source_scan("/")
                        .map_err(plain_libs::anyhow::Error::msg)?;
                }
            },
            Command::Pause => {
                catalog
                    .pause_file_scan()
                    .map_err(plain_libs::anyhow::Error::msg)?;
            }
            Command::Resume => {
                catalog
                    .resume_file_scan()
                    .map_err(plain_libs::anyhow::Error::msg)?;
            }
            Command::Stop => {
                catalog
                    .cancel_file_scan()
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
