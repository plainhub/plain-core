use crate::features::services::files::media_scan_policy::Policy;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::{mpsc, watch};

pub(super) fn entries(
    root: PathBuf,
    data_directory: PathBuf,
    policy: Arc<Policy>,
    cancelled: watch::Receiver<bool>,
    stopped: watch::Receiver<bool>,
) -> (
    mpsc::Receiver<PathBuf>,
    tokio::task::JoinHandle<Result<Vec<PathBuf>, String>>,
) {
    let (send, receive) = mpsc::channel(32);
    let task = tokio::task::spawn_blocking(move || {
        let mut excluded_roots = Vec::new();
        plain_libs::file_walk::walk_entries(
            &root,
            |path| {
                if let Some(root) = crate::features::media::catalog_paths::scan_excluded_root(path)
                {
                    if !excluded_roots.iter().any(|existing| existing == root) {
                        excluded_roots.push(root.to_owned());
                    }
                    return true;
                }
                path.to_str().is_some_and(|path| {
                    policy.scope.as_ref().map_or_else(
                        || {
                            crate::features::media::catalog_paths::is_file_excluded_at(
                                path,
                                &data_directory,
                                &policy.cache_directory,
                                &policy.excluded_roots,
                            )
                        },
                        |scope| !scope.contains(std::path::Path::new(path)),
                    )
                })
            },
            || {
                *cancelled.borrow()
                    || cancelled.has_changed().is_err()
                    || *stopped.borrow()
                    || stopped.has_changed().is_err()
            },
            |path| {
                send.blocking_send(path.to_owned())
                    .map_err(|_| anyhow::anyhow!("File scan receiver closed"))
            },
        )
        .map_err(|error| error.to_string())?;
        Ok(excluded_roots)
    });
    (receive, task)
}
