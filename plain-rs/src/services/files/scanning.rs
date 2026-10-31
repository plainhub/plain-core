use anyhow::Result;
use plain_rs::runtime::Runtime as CoreRuntime;

pub struct Progress {
    pub indexed: i32,
    pub total: Option<i32>,
    pub state: ScanState,
    pub error: Option<String>,
}
/// Media-index scan lifecycle, shared by `scanProgress.state` and the
/// `media:scan:progress` WS payload (same values on both wires).
#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ScanState {
    Idle,
    Running,
    Paused,
    Stopped,
    Failed,
}
pub enum Command {
    Rebuild(#[cfg(feature = "runtime-media")] String),
    Pause,
    Resume,
    Stop,
    SetDirectories(#[cfg(feature = "runtime-media")] Vec<String>),
}

async fn available(state: &CoreRuntime) -> Result<()> {
    plain_rs::services::capabilities::require(&state.host, "MEDIA_SCAN").await
}
pub async fn progress(state: &CoreRuntime) -> Result<Progress> {
    available(state).await?;
    #[cfg(feature = "runtime-media")]
    {
        use plain_rs::services::files::media_scans::Status;
        let Some(snapshot) = state.media_scans.snapshot().map_err(anyhow::Error::msg)? else {
            return Ok(Progress {
                indexed: 0,
                total: Some(0),
                state: ScanState::Idle,
                error: None,
            });
        };
        Ok(Progress {
            indexed: snapshot.processed_files.try_into()?,
            total: snapshot.total_files.map(i32::try_from).transpose()?,
            state: match snapshot.status {
                Status::Running => ScanState::Running,
                Status::Paused => ScanState::Paused,
                Status::Cancelling | Status::Cancelled => ScanState::Stopped,
                Status::Completed => ScanState::Idle,
                Status::Failed => ScanState::Failed,
            },
            error: snapshot.error,
        })
    }
    #[cfg(not(feature = "runtime-media"))]
    anyhow::bail!("MEDIA_SCAN advertised without a scanner implementation")
}
pub async fn directories(state: &CoreRuntime) -> Result<Vec<String>> {
    available(state).await?;
    Ok(plain_rs::prefs::media::source_directories(&state.prefs)?)
}
pub async fn execute(state: &CoreRuntime, command: Command) -> Result<()> {
    available(state).await?;
    #[cfg(feature = "runtime-media")]
    {
        let catalog =
            plain_rs::services::files::catalog_scan::NativeMediaCatalog::new(state.clone());
        match command {
            Command::Rebuild(root) => {
                catalog
                    .start_source_scan(&root)
                    .map_err(anyhow::Error::msg)?;
            }
            Command::Pause => {
                catalog.pause_file_scan().map_err(anyhow::Error::msg)?;
            }
            Command::Resume => {
                catalog.resume_file_scan().map_err(anyhow::Error::msg)?;
            }
            Command::Stop => {
                catalog.cancel_file_scan().map_err(anyhow::Error::msg)?;
            }
            Command::SetDirectories(dirs) => catalog
                .set_source_directories(&dirs)
                .map_err(anyhow::Error::msg)?,
        }
        Ok(())
    }
    #[cfg(not(feature = "runtime-media"))]
    {
        let _ = command;
        anyhow::bail!("MEDIA_SCAN advertised without a scanner implementation")
    }
}
