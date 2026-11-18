use crate::features::capability::Capability;
use crate::features::enums::DataType;
use crate::features::files::trash::Trash;
use crate::features::media::actions::Action;
use crate::features::media::actions::Item;
use crate::features::media::actions::Outcome;
use crate::infrastructure::db::Db;
use crate::infrastructure::error::DomainError;
use crate::infrastructure::error::DomainResult;
use crate::infrastructure::host::bridge::Host;
use plain_libs::rusqlite::OptionalExtension;
use std::path::{Path, PathBuf};
#[cfg(feature = "runtime-media")]
use std::sync::Arc;

#[cfg(any(target_os = "android", target_os = "ios"))]
const FILESYSTEM: &[Capability] = &[];
#[cfg(not(any(target_os = "android", target_os = "ios")))]
const FILESYSTEM: &[Capability] = &[Capability::MediaDelete, Capability::MediaMove];

pub fn capabilities() -> &'static [Capability] {
    FILESYSTEM
}

pub async fn capability_list(host: &Host) -> Result<Vec<Capability>, String> {
    if cfg!(any(target_os = "android", target_os = "ios")) {
        crate::features::services::capabilities::snapshot(host)
            .await
            .map_err(|error| error.to_string())
    } else {
        Ok(capabilities().to_vec())
    }
}

pub fn action_capability(action: Action) -> Capability {
    match action {
        Action::Delete => Capability::MediaDelete,
        Action::Move => Capability::MediaMove,
        Action::Trash | Action::Restore => Capability::MediaTrash,
    }
}

enum Backend {
    Provider(Outcome),
    Files {
        from_trash: bool,
        destination: String,
        trash: Option<Trash>,
        #[cfg(feature = "runtime-media")]
        scans: Arc<crate::features::services::files::media_scans::MediaScans>,
    },
}
pub struct Operation {
    kind: DataType,
    action: Action,
    ids: Vec<String>,
    backend: Backend,
}

impl Operation {
    pub async fn prepare(
        host: &Host,
        platform: &crate::features::services::files::platform::FilePlatform,
        kind: DataType,
        action: Action,
        ids: Vec<String>,
        from_trash: bool,
        gallery_trash: bool,
        destination: String,
        #[cfg(feature = "runtime-media")] scans: Arc<
            crate::features::services::files::media_scans::MediaScans,
        >,
    ) -> Result<Self, String> {
        let backend = if host.uses_system_media_catalog() {
            Backend::Provider(
                crate::features::services::files::media_provider::run(
                    host,
                    platform,
                    kind,
                    action,
                    &ids,
                    from_trash,
                    &destination,
                )
                .await?,
            )
        } else {
            Backend::Files {
                from_trash,
                destination,
                trash: gallery_trash.then(|| Trash::new(platform.db.clone())),
                #[cfg(feature = "runtime-media")]
                scans,
            }
        };
        Ok(Self {
            kind,
            action,
            ids,
            backend,
        })
    }

    pub fn complete<T>(
        self,
        db: &Db,
        finish: impl FnOnce(Outcome) -> DomainResult<T>,
    ) -> DomainResult<T> {
        let commit = |outcome| {
            crate::features::media::actions::validate(self.kind, self.action, &self.ids, &outcome)?;
            finish(outcome)
        };
        match self.backend {
            Backend::Provider(outcome) => commit(outcome),
            Backend::Files {
                from_trash,
                destination,
                trash,
                #[cfg(feature = "runtime-media")]
                scans,
            } => {
                let run = || {
                    commit(files(
                        db,
                        self.kind,
                        self.action,
                        &self.ids,
                        from_trash,
                        &destination,
                        trash.as_ref(),
                    )?)
                };
                #[cfg(feature = "runtime-media")]
                {
                    scans
                        .record_completion(|| Ok(run()))
                        .map_err(DomainError::Other)?
                }
                #[cfg(not(feature = "runtime-media"))]
                {
                    run()
                }
            }
        }
    }
}

fn files(
    db: &Db,
    kind: DataType,
    action: Action,
    ids: &[String],
    from_trash: bool,
    destination: &str,
    trash: Option<&Trash>,
) -> DomainResult<Outcome> {
    if !matches!(action, Action::Delete | Action::Move) && trash.is_none() {
        return Err(DomainError::Other(
            "Unsupported filesystem media operation".into(),
        ));
    }
    let destination = if action == Action::Move {
        let path = PathBuf::from(destination);
        crate::features::services::files::platform::authorize_native(&path, true)
            .map_err(|error| DomainError::Other(error.to_string()))?;
        std::fs::create_dir_all(&path).map_err(|error| DomainError::Other(error.to_string()))?;
        Some(
            path.canonicalize()
                .map_err(|error| DomainError::Other(error.to_string()))?,
        )
    } else {
        None
    };
    let rows = db.with_conn(|connection| {
        let mut statement = connection
            .prepare("SELECT path,trashed FROM file_catalog WHERE id=?1 AND data_type=?2")?;
        ids.iter()
            .map(|id| {
                statement
                    .query_row(plain_libs::rusqlite::params![id, kind.kind()], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?))
                    })
                    .optional()
            })
            .collect::<plain_libs::rusqlite::Result<Vec<_>>>()
    })?;
    let mut outcome = Outcome {
        successful: Vec::new(),
        failed_ids: Vec::new(),
    };
    for (id, row) in ids.iter().zip(rows) {
        let changed = (|| -> Result<Item, String> {
            let (path, trashed) = row.ok_or("Media file not found")?;
            let source = Path::new(&path);
            if !source.is_absolute()
                || !std::fs::symlink_metadata(source)
                    .map_err(|error| error.to_string())?
                    .file_type()
                    .is_file()
            {
                return Err("Regular media file required".into());
            }
            crate::features::services::files::platform::authorize_native(source, true)
                .map_err(|error| error.to_string())?;
            let destination_path = match action {
                Action::Delete => {
                    if trashed != from_trash {
                        return Err("Media trash state changed".into());
                    }
                    std::fs::remove_file(source).map_err(|error| error.to_string())?;
                    String::new()
                }
                Action::Move => {
                    if trashed {
                        return Err("Trashed media is not movable".into());
                    }
                    let target = destination.as_ref().ok_or("Media destination required")?;
                    if source
                        .parent()
                        .ok_or("Media source parent required")?
                        .canonicalize()
                        .map_err(|error| error.to_string())?
                        == *target
                    {
                        return Err("Media parent unchanged".into());
                    }
                    let base = target.join(source.file_name().ok_or("Media file name required")?);
                    loop {
                        let candidate = plain_libs::utils::unique_path::unique_sibling(&base)
                            .map_err(|error| error.to_string())?;
                        match tokio::runtime::Handle::current()
                            .block_on(plain_libs::filesystem::rename(source, &candidate))
                        {
                            Ok(()) => break candidate.to_string_lossy().into_owned(),
                            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                                continue;
                            }
                            Err(error) => return Err(error.to_string()),
                        }
                    }
                }
                Action::Trash => {
                    if trashed {
                        return Err("Media is already in trash".into());
                    }
                    let store = trash.ok_or("Unsupported filesystem media operation")?;
                    tokio::runtime::Handle::current()
                        .block_on(store.trash_paths(vec![path.clone()]))
                        .map_err(|error| error.to_string())?
                        .pop()
                        .ok_or("Missing trash destination")?
                }
                Action::Restore => {
                    if !trashed {
                        return Err("Media is not in trash".into());
                    }
                    let store = trash.ok_or("Unsupported filesystem media operation")?;
                    let item = store.load_item(id)
                        .map_err(|error| error.to_string())?
                        .ok_or("Media trash item not found")?;
                    crate::features::services::files::platform::authorize_native(
                        Path::new(&item.original_path), true,
                    ).map_err(|error| error.to_string())?;
                    tokio::runtime::Handle::current()
                        .block_on(store.restore_paths(vec![id.clone()]))
                        .map_err(|error| error.to_string())?
                        .pop()
                        .ok_or("Missing restore destination")?
                }
            };
            Ok(Item {
                id: id.clone(),
                path,
                destination_path,
            })
        })();
        match changed {
            Ok(item) => outcome.successful.push(item),
            Err(error) => {
                log::warn!("Media operation failed for {id}: {error}");
                outcome.failed_ids.push(id.clone());
            }
        }
    }
    Ok(outcome)
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/files/media_action_io.rs"]
mod tests;
