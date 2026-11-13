use super::*;
use std::{collections::HashSet, path::PathBuf};

pub(super) struct Target {
    pub entry: Entry,
    pub relative: PathBuf,
}
pub(super) struct Plan {
    pub targets: Vec<Target>,
    pub directories: Vec<PathBuf>,
    pub title: String,
}
pub(super) async fn build<F, Work>(urls: &[String], fetch: F) -> Result<Plan>
where
    F: Fn(String) -> Work,
    Work: std::future::Future<Output = Result<Metadata>>,
{
    let mut pending = vec![];
    for url in urls.iter().rev() {
        let info = fetch(url.clone()).await?;
        pending.push((info.entry, Some(info.children), PathBuf::new(), 0));
    }
    let title = pending
        .last()
        .expect("Selected resource exists")
        .0
        .name
        .clone();
    let mut directories = vec![];
    let mut paths = HashSet::new();
    let mut targets = vec![];
    let mut nodes = 0;
    while let Some((entry, children, parent, depth)) = pending.pop() {
        nodes += 1;
        ensure!(
            nodes <= 100_000 && depth < 64,
            "Download directory capacity exceeded"
        );
        transfer::validate_name(&entry.name)?;
        let relative = parent.join(&entry.name);
        ensure!(
            paths.insert(relative.clone()),
            "Duplicate download destination"
        );
        if entry.is_dir {
            ensure!(
                directories.len() < 10_000,
                "Download directory capacity exceeded"
            );
            directories.push(relative.clone());
            let children = match children {
                Some(children) => children,
                None => {
                    let info = fetch(entry.url.clone()).await?;
                    ensure!(info.entry.is_dir, "Download directory changed");
                    info.children
                }
            };
            for child in children.into_iter().rev() {
                pending.push((child, None, relative.clone(), depth + 1));
            }
        } else {
            targets.push(Target { entry, relative });
        }
    }
    Ok(Plan {
        targets,
        directories,
        title,
    })
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-rs/unit/features/services/downloads/resources/plan.rs"]
mod tests;
