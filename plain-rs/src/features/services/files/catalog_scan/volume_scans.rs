use crate::features::files::index_scope::{Scope, source_directories};
use crate::infrastructure::db::{Db, FileIndexScanStatus};
use std::path::Path;

struct ScanRoot {
    path: String,
    discovered: bool,
    completed: bool,
}

struct VolumeScan {
    id: String,
    path: String,
    roots: Vec<ScanRoot>,
    count: i64,
    total: i64,
    failed: bool,
}

#[derive(Default)]
pub(super) struct VolumeScans(Vec<VolumeScan>);

impl VolumeScans {
    pub(super) fn new(scope: Option<&Scope>, references: &[String], background: bool) -> Self {
        if background {
            return Self::default();
        }
        let Some(scope) = scope else {
            return Self::default();
        };
        Self(
            scope
                .volumes
                .iter()
                .filter_map(|volume| {
                    let roots = source_directories(std::slice::from_ref(volume));
                    if roots.is_empty() || !roots.iter().all(|root| references.contains(root)) {
                        return None;
                    }
                    Some(VolumeScan {
                        id: volume.id.clone(),
                        path: volume.path.clone(),
                        roots: roots
                            .into_iter()
                            .map(|path| ScanRoot {
                                path,
                                discovered: false,
                                completed: false,
                            })
                            .collect(),
                        count: 0,
                        total: 0,
                        failed: false,
                    })
                })
                .collect(),
        )
    }

    fn volume(&mut self, path: &Path) -> Option<&mut VolumeScan> {
        self.0
            .iter_mut()
            .filter(|volume| path.starts_with(&volume.path))
            .max_by_key(|volume| volume.path.len())
    }

    pub(super) fn discovered_file(&mut self, path: &Path) -> Result<(), String> {
        if let Some(volume) = self.volume(path) {
            volume.total = volume
                .total
                .checked_add(1)
                .ok_or("Volume scan total overflow")?;
        }
        Ok(())
    }

    pub(super) fn processed(
        &mut self,
        paths: impl IntoIterator<Item = impl AsRef<Path>>,
    ) -> Result<(), String> {
        for path in paths {
            if let Some(volume) = self.volume(path.as_ref()) {
                volume.count = volume
                    .count
                    .checked_add(1)
                    .ok_or("Volume scan count overflow")?;
            }
        }
        Ok(())
    }

    pub(super) fn discovered_root(&mut self, id: &str, root: &Path) {
        for volume in self.0.iter_mut().filter(|volume| volume.id == id) {
            for entry in volume
                .roots
                .iter_mut()
                .filter(|entry| Path::new(&entry.path).starts_with(root))
            {
                entry.discovered = true;
            }
        }
    }

    pub(super) fn completed_root(&mut self, id: &str, root: &Path) {
        for volume in self.0.iter_mut().filter(|volume| volume.id == id) {
            for entry in volume
                .roots
                .iter_mut()
                .filter(|entry| Path::new(&entry.path).starts_with(root))
            {
                entry.completed = true;
            }
        }
    }

    pub(super) fn failed(&mut self, path: &Path) {
        if let Some(volume) = self.volume(path) {
            volume.failed = true;
        }
    }

    pub(super) fn persist(self, db: &Db, canceled: bool) -> Result<(), String> {
        for volume in self.0 {
            let status = if volume.failed {
                FileIndexScanStatus::Failed
            } else if volume.roots.iter().all(|root| root.completed) {
                FileIndexScanStatus::Completed
            } else if canceled {
                FileIndexScanStatus::Canceled
            } else {
                FileIndexScanStatus::Failed
            };
            let total = volume
                .roots
                .iter()
                .all(|root| root.discovered)
                .then_some(volume.total);
            db.record_volume_scan(&volume.id, status, volume.count, total)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-rs/unit/features/services/files/catalog_scan/volume_scans.rs"]
mod tests;
