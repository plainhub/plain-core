use crate::features::files::resource_link::{self, Resource, ResourceType};
use crate::features::services::files::links::download_url;
use crate::runtime::Runtime as CoreRuntime;
use plain_libs::anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Entry {
    pub url: String,
    pub name: String,
    pub is_dir: bool,
    pub size_bytes: i64,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub entry: Entry,
    pub children: Vec<Entry>,
}

pub async fn describe(state: &CoreRuntime, id: &str) -> Result<Metadata> {
    use crate::features::services::files::links;
    let (scope, payload) = resource_link::parse(id)?;
    let key = links::key(state, &scope)?;
    let resource = resource_link::open(&scope, &key, payload)?;
    let file = links::resolve(state, id, links::ReadKind::Original).await?;
    let name = if file.name.is_empty() {
        file.path.rsplit('/').next().unwrap_or_default().to_owned()
    } else {
        file.name
    };
    if file.path.starts_with("content://")
        || file.path.starts_with("ph://")
        || file.path.contains("!zip!/")
    {
        let path = if file.path.contains("!zip!/") {
            state
                .archives
                .preview(&state.file_platform, file.path)
                .await
                .map_err(plain_libs::anyhow::Error::msg)?
        } else {
            let staged = crate::features::services::files::source_files::stage(
                &file.path,
                state.db.clone(),
                &state.directory,
                &state.resources,
                state.cache.root(),
                state.stop.clone(),
            )
            .await
            .map_err(plain_libs::anyhow::Error::msg)?;
            let size = i64::try_from(staged.as_file().metadata()?.len())?;
            return Ok(Metadata {
                entry: Entry {
                    url: download_url(id),
                    name,
                    is_dir: false,
                    size_bytes: size,
                },
                children: vec![],
            });
        };
        return Ok(Metadata {
            entry: Entry {
                url: download_url(id),
                name,
                is_dir: false,
                size_bytes: i64::try_from(tokio::fs::metadata(path).await?.len())?,
            },
            children: vec![],
        });
    }
    let path = std::path::PathBuf::from(&file.path);
    let metadata = tokio::fs::metadata(&path).await?;
    ensure!(
        metadata.is_file() || metadata.is_dir(),
        "Unsupported download resource"
    );
    let entry = Entry {
        url: download_url(id),
        name,
        is_dir: metadata.is_dir(),
        size_bytes: if metadata.is_file() {
            i64::try_from(metadata.len())?
        } else {
            0
        },
    };
    let children = if entry.is_dir {
        let parent_resource = resource.clone();
        tokio::task::spawn_blocking(move || -> Result<Vec<Entry>> {
            let mut entries = vec![];
            for child in std::fs::read_dir(path)? {
                let child = child?;
                let metadata = child.path().symlink_metadata()?;
                if metadata.file_type().is_symlink() {
                    continue;
                }
                ensure!(
                    entries.len() < 100_000,
                    "Download directory capacity exceeded"
                );
                let name = child
                    .file_name()
                    .into_string()
                    .map_err(|_| plain_libs::anyhow::anyhow!("Invalid filename encoding"))?;
                let id = if parent_resource.r#type == ResourceType::SharedFile {
                    format!("{}/{}", parent_resource.id.trim_end_matches('/'), name)
                        .trim_start_matches('/')
                        .to_owned()
                } else {
                    child
                        .path()
                        .to_str()
                        .ok_or_else(|| plain_libs::anyhow::anyhow!("Invalid file path encoding"))?
                        .into()
                };
                let child = Resource {
                    r#type: parent_resource.r#type,
                    id,
                };
                let id = resource_link::seal(&scope, &key, &child)?;
                entries.push(Entry {
                    url: download_url(&id),
                    name,
                    is_dir: metadata.is_dir(),
                    size_bytes: if metadata.is_file() {
                        i64::try_from(metadata.len())?
                    } else {
                        0
                    },
                });
            }
            entries.sort_by(|a, b| a.name.cmp(&b.name));
            Ok(entries)
        })
        .await??
    } else {
        vec![]
    };
    Ok(Metadata { entry, children })
}
