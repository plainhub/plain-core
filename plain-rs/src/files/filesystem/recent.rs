use super::{FileEntry, walk::Walk};
use std::{cmp::Reverse, collections::BinaryHeap, path::Path, time::SystemTime};

pub fn list(root: &Path, limit: usize) -> Vec<FileEntry> {
    if limit == 0 {
        return Vec::new();
    }
    let mut newest = BinaryHeap::new();
    for entry in Walk::new(root).into_iter().filter_map(Result::ok) {
        let metadata = if entry.file_type().is_symlink() {
            std::fs::metadata(entry.path())
        } else {
            entry.metadata()
        };
        let Ok(metadata) = metadata else { continue };
        if !metadata.is_file() {
            continue;
        }
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        let Ok(size) = i64::try_from(metadata.len()) else {
            continue;
        };
        let candidate = Reverse((modified, entry.path().to_path_buf(), size));
        if newest.len() < limit {
            newest.push(candidate);
        } else if newest.peek().is_some_and(|oldest| candidate.0 > oldest.0) {
            newest.pop();
            newest.push(candidate);
        }
    }
    let mut rows = newest
        .into_iter()
        .map(|Reverse(row)| row)
        .collect::<Vec<_>>();
    rows.sort_unstable_by(|left, right| right.cmp(left));
    rows.into_iter()
        .map(|(modified, path, size): (SystemTime, _, _)| {
            let updated_at = modified.into();
            FileEntry {
                path: path
                    .to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/"),
                is_dir: false,
                created_at: updated_at,
                updated_at,
                size,
                child_count: 0,
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/files/filesystem/recent.rs"]
mod tests;
