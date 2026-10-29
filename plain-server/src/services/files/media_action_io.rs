use crate::host::bridge::Host;
use plain_rs::{
    db::Db,
    enums::DataType,
    error::{DomainError, DomainResult},
    media::actions::{Action, Item, Outcome},
};
use rusqlite::OptionalExtension;
use std::path::{Path, PathBuf};
#[cfg(feature = "media")]
use std::sync::Arc;

pub fn capabilities() -> &'static [&'static str] {
    if cfg!(any(target_os = "android", target_os = "ios")) {
        &[]
    } else {
        &["MEDIA_DELETE", "MEDIA_MOVE"]
    }
}

pub(crate) async fn capability_list(host: &Host) -> Result<Vec<String>, String> {
    if cfg!(any(target_os = "android", target_os = "ios")) {
        crate::services::capabilities::snapshot(host)
            .await
            .map_err(|error| error.to_string())
    } else {
        Ok(capabilities()
            .iter()
            .map(|value| (*value).to_owned())
            .collect())
    }
}

pub(crate) fn action_capability(action: Action) -> &'static str {
    match action {
        Action::Delete => "MEDIA_DELETE",
        Action::Move => "MEDIA_MOVE",
        Action::Trash | Action::Restore => "MEDIA_TRASH",
    }
}

enum Backend {
    Provider(Outcome),
    Files {
        from_trash: bool,
        destination: String,
        #[cfg(feature = "media")]
        scans: Arc<super::media_scans::MediaScans>,
    },
}
pub(crate) struct Operation {
    kind: DataType,
    action: Action,
    ids: Vec<String>,
    backend: Backend,
}

impl Operation {
    pub(crate) async fn prepare(
        host: &Host,
        platform: &super::platform::FilePlatform,
        kind: DataType,
        action: Action,
        ids: Vec<String>,
        from_trash: bool,
        destination: String,
        #[cfg(feature = "media")] scans: Arc<super::media_scans::MediaScans>,
    ) -> Result<Self, String> {
        let backend = if host.uses_system_media_catalog() {
            Backend::Provider(
                super::media_provider::run(
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
                #[cfg(feature = "media")]
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

    pub(crate) fn complete<T>(
        self,
        db: &Db,
        finish: impl FnOnce(Outcome) -> DomainResult<T>,
    ) -> DomainResult<T> {
        let commit = |outcome| {
            plain_rs::media::actions::validate(self.kind, self.action, &self.ids, &outcome)?;
            finish(outcome)
        };
        match self.backend {
            Backend::Provider(outcome) => commit(outcome),
            Backend::Files {
                from_trash,
                destination,
                #[cfg(feature = "media")]
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
                    )?)
                };
                #[cfg(feature = "media")]
                {
                    scans
                        .record_completion(|| Ok(run()))
                        .map_err(DomainError::Other)?
                }
                #[cfg(not(feature = "media"))]
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
) -> DomainResult<Outcome> {
    if !matches!(action, Action::Delete | Action::Move) {
        return Err(DomainError::Other(
            "Unsupported filesystem media operation".into(),
        ));
    }
    let destination = if action == Action::Move {
        let path = PathBuf::from(destination);
        super::platform::authorize_native(&path, true)
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
                    .query_row(rusqlite::params![id, kind.kind()], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?))
                    })
                    .optional()
            })
            .collect::<rusqlite::Result<Vec<_>>>()
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
            super::platform::authorize_native(source, true).map_err(|error| error.to_string())?;
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
                    if plain_rs::platform::mountinfo::resolve_mount_point(source)
                        .map_err(|error| error.to_string())?
                        != plain_rs::platform::mountinfo::resolve_mount_point(target)
                            .map_err(|error| error.to_string())?
                    {
                        return Err("Media move must stay in its storage volume".into());
                    }
                    let base = target.join(source.file_name().ok_or("Media file name required")?);
                    loop {
                        let candidate = plain_rs::utils::unique_path::unique_sibling(&base)
                            .map_err(|error| error.to_string())?;
                        match plain_rs::files::filesystem::rename::no_replace(source, &candidate) {
                            Ok(()) => break candidate.to_string_lossy().into_owned(),
                            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                                continue;
                            }
                            Err(error) => return Err(error.to_string()),
                        }
                    }
                }
                _ => return Err("Unsupported filesystem media operation".into()),
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

#[cfg(all(test, not(any(target_os = "android", target_os = "ios"))))]
#[path = "../../../tests/unit/services/files/media_action_io.rs"]
mod tests;
