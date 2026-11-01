use std::path::{Path, PathBuf};

pub fn prune_nested(mut roots: Vec<PathBuf>, keep_nested: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
    roots.retain(|root| !root.as_os_str().is_empty());
    roots.sort_by(|left, right| {
        left.components()
            .count()
            .cmp(&right.components().count())
            .then_with(|| left.cmp(right))
    });
    roots.dedup();
    let mut distinct: Vec<PathBuf> = Vec::new();
    for root in roots {
        if keep_nested(&root) || !distinct.iter().any(|parent| root.starts_with(parent)) {
            distinct.push(root);
        }
    }
    distinct
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/filesystem/roots.rs"]
mod tests;
