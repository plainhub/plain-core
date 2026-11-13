use super::*;
use crate::features::downloads::{Source, queue::Run};
use std::path::PathBuf;

pub(crate) async fn execute(
    state: &CoreRuntime,
    run: &Run,
    work: &super::super::io::BlockingWork,
    canceled: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<()> {
    let Source::Resources { urls } = &run.download.source else {
        unreachable!()
    };
    let plan = plan::build(urls, |url| async move {
        metadata(state, &url, &run.peers).await
    })
    .await?;
    let targets = &plan.targets;
    for (path, size) in &run.completed {
        ensure!(
            targets
                .iter()
                .any(|target| target.relative.to_str() == Some(path.as_str())
                    && target.entry.size_bytes == *size),
            "Completed download source changed"
        );
    }
    let total = targets.iter().try_fold(0_i64, |sum, target| {
        sum.checked_add(target.entry.size_bytes)
            .ok_or_else(|| plain_libs::anyhow::anyhow!("Download size overflow"))
    })?;
    state.downloads.plan(
        &run.download.id,
        &run.generation,
        i32::try_from(targets.len())?,
        total,
        run.download.zip_name.as_deref().unwrap_or(&plan.title),
    )?;
    if run.download.zip_name.is_none() {
        for directory in &plan.directories {
            let path = PathBuf::from(&run.download.target_dir).join(directory);
            state
                .file_platform
                .authorize(
                    path.to_str()
                        .ok_or_else(|| plain_libs::anyhow::anyhow!("Invalid directory encoding"))?,
                    true,
                )
                .await?;
            tokio::fs::create_dir_all(path).await?;
        }
    }
    let workspace = run
        .download
        .zip_name
        .as_ref()
        .map(|_| plain_libs::temporary::Builder::new().tempdir_in(state.cache.root()))
        .transpose()?;
    let mut items = if run.download.zip_name.is_some() {
        plan.directories
            .iter()
            .map(|path| {
                Ok(super::super::archive::Item {
                    source_path: workspace
                        .as_ref()
                        .expect("ZIP workspace created")
                        .path()
                        .to_owned(),
                    entry_name: path
                        .to_str()
                        .ok_or_else(|| {
                            plain_libs::anyhow::anyhow!("Invalid archive path encoding")
                        })?
                        .into(),
                })
            })
            .collect::<Result<Vec<_>>>()?
    } else {
        vec![]
    };
    for (index, target) in targets.iter().enumerate() {
        let key = target
            .relative
            .to_str()
            .ok_or_else(|| plain_libs::anyhow::anyhow!("Invalid download path"))?;
        if let Some(size) = run.completed.get(key) {
            ensure!(
                *size == target.entry.size_bytes,
                "Completed download source changed"
            );
            continue;
        }
        state.downloads.begin_file(
            &run.download.id,
            &run.generation,
            &target.entry.name,
            target.entry.size_bytes,
        )?;
        let destination = if run.download.zip_name.is_some() {
            workspace
                .as_ref()
                .expect("ZIP workspace created")
                .path()
                .join(index.to_string())
        } else {
            PathBuf::from(&run.download.target_dir).join(&target.relative)
        };
        let result = transfer::save(
            state,
            run,
            work,
            &target.entry,
            key,
            &destination,
            run.download.zip_name.is_none(),
        )
        .await;
        match result {
            Ok(path) => {
                if run.download.zip_name.is_some() {
                    items.push(super::super::archive::Item {
                        source_path: path.clone(),
                        entry_name: key.into(),
                    });
                }
                if run.download.zip_name.is_none() {
                    state
                        .file_platform
                        .scan(&[path.to_string_lossy().into_owned()])
                        .await?;
                }
            }
            Err(error) => {
                if run.download.zip_name.is_some() {
                    return Err(error);
                }
                state
                    .downloads
                    .fail_file(&run.download.id, &run.generation, key, error)?;
            }
        }
    }
    if let Some(name) = &run.download.zip_name {
        state.downloads.packing(&run.download.id, &run.generation)?;
        transfer::pack(
            state,
            run,
            work,
            items,
            &PathBuf::from(&run.download.target_dir).join(name),
            canceled,
        )
        .await?;
    }
    Ok(())
}
