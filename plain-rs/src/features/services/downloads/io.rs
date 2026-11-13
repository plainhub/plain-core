use crate::runtime::Runtime as CoreRuntime;
use plain_libs::anyhow::{Result, ensure};
use std::path::{Path, PathBuf};

#[derive(Default)]
pub(crate) struct BlockingWork {
    active: tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
}
impl BlockingWork {
    async fn join(active: &mut Option<tokio::task::JoinHandle<()>>) -> Result<()> {
        let result = match active.as_mut() {
            Some(job) => job.await.map_err(plain_libs::anyhow::Error::from),
            None => Ok(()),
        };
        *active = None;
        result
    }
    pub(crate) async fn run<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        {
            let mut active = self.active.lock().await;
            Self::join(&mut active).await?;
            *active = Some(tokio::task::spawn_blocking(move || {
                let _ = sender.send(work());
            }));
        }
        match receiver.await {
            Ok(result) => result,
            Err(error) => {
                self.finish().await?;
                Err(error.into())
            }
        }
    }
    pub(crate) async fn finish(&self) -> Result<()> {
        let mut active = self.active.lock().await;
        Self::join(&mut active).await
    }
}

pub(crate) struct Temporary(pub(crate) PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        if !self.0.as_os_str().is_empty() {
            if let Err(error) = std::fs::remove_file(&self.0) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    log::error!("Download staging cleanup failed: {error}");
                }
            }
        }
    }
}
impl Temporary {
    pub(crate) fn keep(mut self) {
        self.0.clear();
    }
}
pub(crate) async fn create_file(
    work: &BlockingWork,
    path: PathBuf,
) -> Result<(tokio::fs::File, Temporary)> {
    work.run(move || {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        Ok((tokio::fs::File::from_std(file), Temporary(path)))
    })
    .await
}
pub(crate) fn commit(
    source: PathBuf,
    target: PathBuf,
    reservation: Option<Temporary>,
) -> Result<()> {
    std::fs::rename(source, target)?;
    if let Some(reservation) = reservation {
        reservation.keep();
    }
    Ok(())
}
pub(crate) async fn prepare(
    state: &CoreRuntime,
    work: &BlockingWork,
    path: &Path,
) -> Result<PathBuf> {
    ensure!(path.is_absolute(), "Invalid download destination");
    state
        .file_platform
        .authorize(
            path.to_str()
                .ok_or_else(|| plain_libs::anyhow::anyhow!("Invalid download path"))?,
            true,
        )
        .await?;
    let parent = path
        .parent()
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Missing download destination parent"))?;
    let parent_directory = parent.to_path_buf();
    work.run(move || Ok(std::fs::create_dir_all(parent_directory)?))
        .await?;
    Ok(parent.to_owned())
}

pub(crate) async fn destination(
    work: &BlockingWork,
    path: &Path,
    unique: bool,
) -> Result<(PathBuf, Option<Temporary>)> {
    let parent = path.parent().expect("Download destination prepared");
    if !unique {
        return Ok((path.to_path_buf(), None));
    }
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| plain_libs::anyhow::anyhow!("Invalid download file name"))?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{value}"))
        .unwrap_or_default();
    for number in 0..100_000 {
        let candidate = if number == 0 {
            path.to_path_buf()
        } else {
            parent.join(format!("{stem} ({number}){extension}"))
        };
        match create_file(work, candidate.clone()).await {
            Ok((file, reservation)) => {
                drop(file);
                return Ok((candidate, Some(reservation)));
            }
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|error| error.kind() == std::io::ErrorKind::AlreadyExists) => {}
            Err(error) => return Err(error),
        }
    }
    plain_libs::anyhow::bail!("Shared destination name capacity exceeded")
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/downloads/io.rs"]
mod tests;
