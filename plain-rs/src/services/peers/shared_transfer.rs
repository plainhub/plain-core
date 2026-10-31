use anyhow::{Result, ensure};
use futures_util::StreamExt;
use plain_rs::files::shares::{
    batch_queue::{Receipt, Run},
    client::File,
};
use plain_rs::runtime::Runtime as CoreRuntime;
use serde::Deserialize;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};
use tokio::io::AsyncWriteExt;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Target {
    entry: File,
    write_dir: PathBuf,
    store_to_downloads: bool,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum Operation {
    File {
        url: String,
        target: Target,
        temporary: bool,
    },
    Zip {
        items: Vec<plain_rs::services::peers::shared_zip::Item>,
        path: PathBuf,
    },
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        if !self.0.as_os_str().is_empty() {
            let _ = std::fs::remove_file(&self.0);
        }
    }
}
struct Saved {
    receipt: Receipt,
    committed: Option<Temporary>,
}
impl Temporary {
    fn keep(mut self) {
        self.0.clear();
    }
}
async fn create_file(path: PathBuf) -> Result<(tokio::fs::File, Temporary)> {
    tokio::task::spawn_blocking(move || {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        Ok((tokio::fs::File::from_std(file), Temporary(path)))
    })
    .await?
}
async fn commit(source: PathBuf, target: PathBuf) -> Result<Temporary> {
    tokio::task::spawn_blocking(move || {
        std::fs::rename(source, &target)?;
        Ok(Temporary(target))
    })
    .await?
}
pub fn directory(root: &Path, generation: &str) -> Result<PathBuf> {
    uuid::Uuid::parse_str(generation)?;
    Ok(root.join("shared-transfers").join(generation))
}
pub async fn cleanup(root: &Path, generation: &str) -> Result<()> {
    match tokio::fs::remove_dir_all(directory(root, generation)?).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}
async fn destination(
    state: &CoreRuntime,
    path: &Path,
    unique: bool,
) -> Result<(PathBuf, Option<Temporary>)> {
    ensure!(path.is_absolute(), "Invalid shared destination");
    state
        .file_platform
        .authorize(
            path.to_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid shared path"))?,
            true,
        )
        .await?;
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Missing shared destination parent"))?;
    tokio::fs::create_dir_all(parent).await?;
    if !unique {
        return Ok((path.to_path_buf(), None));
    }
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid shared file name"))?;
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
        match create_file(candidate.clone()).await {
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
    anyhow::bail!("Shared destination name capacity exceeded")
}
#[derive(Default)]
struct Progress {
    last: Duration,
}
impl Progress {
    fn update(&mut self, bytes: i64, expected: i64, elapsed: Duration) -> bool {
        if bytes != expected && elapsed.saturating_sub(self.last) < Duration::from_millis(300) {
            return false;
        }
        self.last = elapsed;
        true
    }
}
async fn download(
    state: &CoreRuntime,
    run: &Run,
    ticket: &str,
    url: String,
    path: &Path,
    expected: i64,
) -> Result<(i64, Temporary)> {
    ensure!(expected >= 0, "Invalid shared file size");
    let url = plain_rs::services::peers::shared_download::download_url(&url)?;
    let _permit = state.lan.capacity.clone().acquire_owned().await?;
    let response =
        tokio::time::timeout(Duration::from_secs(10), state.lan.client.get(url).send()).await??;
    ensure!(
        response.status().is_success(),
        "Shared file HTTP {}",
        response.status()
    );
    if let Some(length) = response.content_length() {
        ensure!(length == expected as u64, "Shared file length mismatch");
    }
    let (mut output, temporary) = create_file(path.to_path_buf()).await?;
    let mut stream = response.bytes_stream();
    let mut downloaded = 0_i64;
    let started = std::time::Instant::now();
    let mut progress = Progress::default();
    while let Some(chunk) = tokio::time::timeout(Duration::from_secs(30), stream.next()).await? {
        let chunk = chunk?;
        if chunk.is_empty() {
            continue;
        }
        downloaded = downloaded
            .checked_add(i64::try_from(chunk.len())?)
            .ok_or_else(|| anyhow::anyhow!("Shared download size overflow"))?;
        ensure!(
            downloaded <= expected,
            "Shared download exceeds declared size"
        );
        output.write_all(&chunk).await?;
        if progress.update(downloaded, expected, started.elapsed()) {
            ensure!(
                state.shared_batches.queue.progress(
                    &run.snapshot.id,
                    &run.snapshot.generation,
                    ticket,
                    downloaded
                )?,
                "Shared download expired"
            );
        }
    }
    ensure!(
        downloaded == expected,
        "Shared download ended before declared size"
    );
    output.flush().await?;
    drop(output);
    Ok((downloaded, temporary))
}
async fn perform(
    state: &CoreRuntime,
    run: &Run,
    ticket: &str,
    operation: Operation,
) -> Result<Saved> {
    match operation {
        Operation::File {
            url,
            target,
            temporary,
        } => {
            let workspace = directory(&state.directory, &run.snapshot.generation)?;
            let (target_path, reservation) = if temporary {
                (workspace.join(format!("{ticket}.data")), None)
            } else {
                destination(
                    state,
                    &target.write_dir.join(&target.entry.name),
                    target.store_to_downloads,
                )
                .await?
            };
            let parent = target_path
                .parent()
                .ok_or_else(|| anyhow::anyhow!("Missing shared destination parent"))?;
            tokio::fs::create_dir_all(parent).await?;
            let (bytes, staging) = download(
                state,
                run,
                ticket,
                url,
                &parent.join(format!(".shared-{ticket}.part")),
                target.entry.size,
            )
            .await?;
            let committed = commit(staging.0.clone(), target_path.clone()).await?;
            if let Some(reservation) = reservation {
                reservation.keep();
            }
            Ok(Saved {
                receipt: Receipt {
                    path: target_path.to_string_lossy().into_owned(),
                    bytes,
                    error: None,
                },
                committed: Some(committed),
            })
        }
        Operation::Zip { items, path } => {
            let (path, _) = destination(state, &path, false).await?;
            let parent = path.parent().unwrap().to_path_buf();
            let canceled = Arc::new(AtomicBool::new(false));
            let _cancel = plain_rs::services::peers::shared_zip::Cancel(canceled.clone());
            let (file, temporary) = tokio::task::spawn_blocking(move || {
                plain_rs::services::peers::shared_zip::pack(parent, items, canceled)
            })
            .await??;
            let bytes = i64::try_from(file.metadata()?.len())?;
            drop(file);
            let committed = commit(temporary.0.clone(), path.clone()).await?;
            Ok(Saved {
                receipt: Receipt {
                    path: path.to_string_lossy().into_owned(),
                    bytes,
                    error: None,
                },
                committed: Some(committed),
            })
        }
    }
}
pub async fn operation(
    state: &CoreRuntime,
    run: &Run,
    name: &str,
    size: Option<i64>,
    params: serde_json::Value,
) -> Result<Receipt> {
    let operation = serde_json::from_value(params)?;
    let (ticket, receiver) =
        state
            .shared_batches
            .queue
            .begin(&run.snapshot.id, &run.snapshot.generation, name, size)?;
    let Saved { receipt, committed } = match tokio::time::timeout(
        Duration::from_secs(15 * 60),
        perform(state, run, &ticket, operation),
    )
    .await
    {
        Ok(Ok(saved)) => saved,
        result => Saved {
            receipt: Receipt {
                path: String::new(),
                bytes: 0,
                error: Some(match result {
                    Ok(Err(error)) => error.to_string(),
                    Err(_) => "Shared transfer timed out".into(),
                    _ => unreachable!(),
                }),
            },
            committed: None,
        },
    };
    let accepted = state.shared_batches.queue.receipt(
        &run.snapshot.id,
        &run.snapshot.generation,
        &ticket,
        receipt,
    )?;
    if accepted {
        if let Some(file) = committed {
            file.keep();
        }
    }
    let receipt = receiver.await?;
    if let Some(error) = &receipt.error {
        anyhow::bail!("{error}");
    }
    Ok(receipt)
}

#[cfg(test)]
#[path = "../../../tests/unit/services/peers/shared_transfer.rs"]
mod tests;
