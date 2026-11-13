use super::io::{BlockingWork, commit, create_file, destination, prepare};
use super::{metadata::Entry, resources};
use crate::{features::downloads::queue::Run, runtime::Runtime as CoreRuntime};
use futures_util::{StreamExt, stream::BoxStream};
use plain_libs::anyhow::{Result, ensure};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub fn validate_name(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty() && !matches!(name, "." | "..") && !name.contains(['/', '\\', '\0']),
        "Invalid download filename"
    );
    Ok(())
}

async fn input(
    state: &CoreRuntime,
    entry: &Entry,
    expected: &[crate::infrastructure::db::DPeer],
) -> Result<(
    BoxStream<'static, Result<bytes::Bytes>>,
    Option<Arc<crate::features::services::peers::managed_resources::PeerRoute>>,
)> {
    let url = resources::url(state, &entry.url)?;
    if resources::is_local(state, &url)? {
        let id = url
            .query_pairs()
            .find(|(key, _)| key == "id")
            .unwrap()
            .1
            .into_owned();
        let file = crate::features::services::files::links::resolve(
            state,
            &id,
            crate::features::services::files::links::ReadKind::Original,
        )
        .await?;
        if file.path.starts_with("content://") || file.path.starts_with("ph://") {
            let reader = state
                .resources
                .read_native(&file.path, state.stop.clone())
                .await?;
            return Ok((
                futures_util::stream::try_unfold(reader, |mut reader| async move {
                    let next = reader
                        .read()
                        .await
                        .map_err(plain_libs::anyhow::Error::msg)?;
                    Ok(next.map(|bytes| (bytes::Bytes::from(bytes), reader)))
                })
                .boxed(),
                None,
            ));
        }
        let path = if file.path.contains("!zip!/") {
            state
                .archives
                .preview(&state.file_platform, file.path)
                .await
                .map_err(plain_libs::anyhow::Error::msg)?
        } else {
            file.path.into()
        };
        let reader = tokio::fs::File::open(path).await?;
        return Ok((
            futures_util::stream::try_unfold(reader, |mut reader| async move {
                let mut bytes = vec![0; 64 * 1024];
                let count = reader.read(&mut bytes).await?;
                bytes.truncate(count);
                Ok::<_, plain_libs::anyhow::Error>(if count == 0 {
                    None
                } else {
                    Some((bytes::Bytes::from(bytes), reader))
                })
            })
            .boxed(),
            None,
        ));
    }
    let permit = state.lan.capacity.clone().acquire_owned().await?;
    let read = resources::access::get(
        state,
        &entry.url,
        crate::features::services::files::content::Action::Download,
        expected,
    )
    .await?;
    let response = read.response;
    if let Some(size) = response.content_length() {
        ensure!(
            size == u64::try_from(entry.size_bytes)?,
            "Download length mismatch"
        );
    }
    Ok((
        response
            .bytes_stream()
            .map(move |chunk| {
                let _ = &permit;
                chunk.map_err(plain_libs::anyhow::Error::from)
            })
            .boxed(),
        read.route,
    ))
}

pub async fn save(
    state: &CoreRuntime,
    run: &Run,
    work: &BlockingWork,
    entry: &Entry,
    key: &str,
    target: &Path,
    unique: bool,
) -> Result<PathBuf> {
    let parent = prepare(state, work, target).await?;
    let staging = parent.join(format!(".download-{}.part", run.generation));
    let (mut output, temporary) = create_file(work, staging).await?;
    let (mut stream, route) = input(state, entry, &run.peers).await?;
    let mut bytes = 0_u64;
    while let Some(chunk) = tokio::time::timeout(Duration::from_secs(30), stream.next()).await? {
        let chunk = chunk?;
        if let Some(route) = &route {
            route.check_current(state)?;
        }
        bytes = bytes
            .checked_add(u64::try_from(chunk.len())?)
            .ok_or_else(|| plain_libs::anyhow::anyhow!("Download size overflow"))?;
        ensure!(
            bytes <= u64::try_from(entry.size_bytes)?,
            "Download exceeds declared size"
        );
        output.write_all(&chunk).await?;
        state
            .downloads
            .progress(&run.download.id, &run.generation, bytes)?;
    }
    if let Some(route) = &route {
        route.check_current(state)?;
    }
    ensure!(
        bytes == u64::try_from(entry.size_bytes)?,
        "Incomplete download"
    );
    output.flush().await?;
    output.sync_all().await?;
    drop(output);
    let (target, reservation) = destination(work, target, unique).await?;
    let queue = state.downloads.clone();
    let id = run.download.id.clone();
    let generation = run.generation.clone();
    let key = key.to_owned();
    let path = target.clone();
    work.run(move || {
        queue.finish_file(&id, &generation, &key, || {
            commit(temporary.0.clone(), path, reservation)
        })
    })
    .await?;
    Ok(target)
}

pub async fn pack(
    state: &CoreRuntime,
    run: &Run,
    work: &BlockingWork,
    items: Vec<super::archive::Item>,
    target: &Path,
    cancel: Arc<AtomicBool>,
) -> Result<()> {
    let parent = prepare(state, work, target).await?;
    let (file, temporary) = work
        .run(move || super::archive::pack(parent, items, cancel))
        .await?;
    drop(file);
    let (path, reservation) = destination(work, target, true).await?;
    let queue = state.downloads.clone();
    let id = run.download.id.clone();
    let generation = run.generation.clone();
    let target = path.clone();
    work.run(move || {
        queue.commit(&id, &generation, || {
            commit(temporary.0.clone(), target, reservation)
        })
    })
    .await?;
    state
        .file_platform
        .scan(&[path.to_string_lossy().into_owned()])
        .await?;
    Ok(())
}
