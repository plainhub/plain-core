use anyhow::{Result, anyhow, bail};
use plain_rs::files::moves::Binding;
use plain_rs::services::files::file_task_walk::FileWalker;
use plain_rs::services::files::platform::FilePlatform;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashSet, path::Path};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub media_type: i32,
    #[serde(rename = "mediaId")]
    pub file_id: String,
    pub path: String,
}
#[derive(Serialize, Deserialize)]
struct Snapshot {
    root: String,
    items: Vec<Identity>,
}
async fn lookup(platform: &FilePlatform, paths: &[String]) -> Result<Vec<Identity>> {
    let rows = platform.identities(paths).await?;
    validate(&rows, paths)?;
    Ok(rows)
}

pub fn validate(rows: &[Identity], paths: &[String]) -> Result<()> {
    let mut seen = HashSet::new();
    for row in rows {
        if !matches!(row.media_type, 1 | 2 | 3 | 22 | 24)
            || row.file_id.is_empty()
            || !paths.contains(&row.path)
            || !seen.insert(&row.path)
        {
            bail!("invalid native media identity receipt");
        }
    }
    Ok(())
}

pub async fn normalized(path: &Path) -> Result<String> {
    let parent = tokio::fs::canonicalize(
        path.parent()
            .ok_or_else(|| anyhow!("missing file parent"))?,
    )
    .await?;
    Ok(parent
        .join(
            path.file_name()
                .ok_or_else(|| anyhow!("missing file name"))?,
        )
        .to_str()
        .ok_or_else(|| anyhow!("invalid path encoding"))?
        .to_owned())
}
pub async fn prepare(platform: &FilePlatform, source: &str) -> Result<Value> {
    let root = normalized(Path::new(source)).await?;
    let mut walker = if platform.host.uses_system_media_catalog() {
        FileWalker::new(Path::new(&root))
    } else {
        FileWalker::entries(Path::new(&root))
    };
    let mut batch = Vec::new();
    let mut items = Vec::new();
    while let Some(path) = walker.next().await? {
        batch.push(
            path.to_str()
                .ok_or_else(|| anyhow!("invalid source encoding"))?
                .to_owned(),
        );
        if batch.len() == 128 {
            items.extend(lookup(platform, &batch).await?);
            batch.clear();
        }
    }
    if !batch.is_empty() {
        items.extend(lookup(platform, &batch).await?);
    }
    Ok(serde_json::to_value(Snapshot { root, items })?)
}
pub struct MediaMoveBindings {
    pub bindings: Vec<Binding>,
    pub source_root: String,
    pub destination_root: String,
}
pub fn source_root(snapshot: &Value) -> Result<String> {
    Ok(serde_json::from_value::<Snapshot>(snapshot.clone())?.root)
}

pub async fn resolve(
    platform: &FilePlatform,
    destination: &str,
    snapshot: &Value,
) -> Result<MediaMoveBindings> {
    let snapshot: Snapshot = serde_json::from_value(snapshot.clone())?;
    let destination = normalized(Path::new(destination)).await?;
    let mut bindings = Vec::new();
    for batch in snapshot.items.chunks(128) {
        let paths = batch
            .iter()
            .map(|row| -> Result<String> {
                let relative = Path::new(&row.path).strip_prefix(&snapshot.root)?;
                let path = if relative.as_os_str().is_empty() {
                    Path::new(&destination).to_owned()
                } else {
                    Path::new(&destination).join(relative)
                };
                Ok(path
                    .to_str()
                    .ok_or_else(|| anyhow!("invalid destination encoding"))?
                    .to_owned())
            })
            .collect::<Result<Vec<_>>>()?;
        let rows = lookup(platform, &paths).await?;
        for (source, path) in batch.iter().zip(paths) {
            let target = rows
                .iter()
                .find(|row| row.path == path && row.media_type == source.media_type)
                .ok_or_else(|| anyhow!("moved media identity missing: {path}"))?;
            bindings.push(Binding {
                media_type: source.media_type,
                source_id: source.file_id.clone(),
                destination_id: target.file_id.clone(),
                source_path: source.path.clone(),
                destination_path: path,
            });
        }
    }
    Ok(MediaMoveBindings {
        bindings,
        source_root: snapshot.root,
        destination_root: destination,
    })
}

pub async fn deletion_snapshot(
    platform: &FilePlatform,
    root: String,
    paths: &[String],
) -> Result<Value> {
    let mut items = Vec::new();
    for batch in paths.chunks(128) {
        items.extend(lookup(platform, batch).await?);
    }
    Ok(serde_json::to_value(Snapshot { root, items })?)
}
pub fn deleted_items(
    snapshot: &Value,
    paths: &[String],
) -> Result<Vec<plain_rs::files::deletes::Item>> {
    let snapshot: Snapshot = serde_json::from_value(snapshot.clone())?;
    Ok(snapshot
        .items
        .into_iter()
        .filter(|item| paths.contains(&item.path))
        .map(|item| plain_rs::files::deletes::Item {
            media_type: item.media_type,
            id: item.file_id,
            path: item.path,
        })
        .collect())
}

#[cfg(test)]
#[path = "../../../tests/unit/services/files/file_task_media.rs"]
mod tests;
