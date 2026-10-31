use plain_rs::enums::DataType;
use plain_rs::runtime::Runtime as CoreRuntime;
use plain_rs::services::files::archives::ArchiveReader;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};

const ZIP_SEPARATOR: &str = "!zip!/";
const MAX_ENTRIES: usize = 100_000;

#[derive(Debug)]
pub enum Error {
    Invalid(String),
    Denied(String),
    Missing,
    Failed(String),
    Busy,
}

pub struct Download {
    pub reader: ArchiveReader,
    pub name: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct ZipFilesRequest {
    r#type: String,
    query: String,
    id: String,
    name: String,
}

fn desktop_access_allowed(state: &CoreRuntime) -> bool {
    state.prefs.get_or("desktop_access", true)
}

fn decrypt_url_id(state: &CoreRuntime, id: &str) -> Option<String> {
    String::from_utf8(plain_rs::services::files::links::decrypt(state, id, "zip-files").ok()?).ok()
}

/// `….zip!zip!/inner` → (archive, inner prefix).
fn split_zip_path(path: &str) -> Option<(PathBuf, String)> {
    let position = path.find(ZIP_SEPARATOR)?;
    let archive = PathBuf::from(&path[..position]);
    let inner = path[position + ZIP_SEPARATOR.len()..].trim_matches('/');
    if !archive.is_file() {
        return None;
    }
    Some((archive, inner.to_owned()))
}

fn walk_directory(
    root: &Path,
    prefix: &str,
    out: &mut Vec<(PathBuf, String)>,
) -> anyhow::Result<()> {
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let relative = path
                .strip_prefix(root)
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_else(|_| name.clone());
            let relative = relative.replace('\\', "/");
            let entry_name = if prefix.is_empty() {
                relative
            } else {
                format!("{prefix}/{relative}")
            };
            if path.is_dir() {
                pending.push(path);
            } else if out.len() < MAX_ENTRIES {
                out.push((path, entry_name));
            }
        }
    }
    Ok(())
}

/// Copy the entries under `inner` out of an open archive.
fn collect_archive_entries(
    archive: &Path,
    inner: &str,
    staging: &Staging,
    out: &mut Vec<(PathBuf, String)>,
) -> anyhow::Result<()> {
    let file = std::fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file)?;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index)?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name()?.into_owned();
        let relative = if inner.is_empty() {
            name.clone()
        } else if let Some(rest) = name.strip_prefix(&format!("{inner}/")) {
            rest.to_owned()
        } else {
            continue;
        };
        if relative.is_empty() || relative.contains("..") || out.len() >= MAX_ENTRIES {
            continue;
        }
        let mut target = std::fs::File::create(staging.entry_path(out.len()))?;
        let mut buffer = vec![0; 64 * 1024];
        loop {
            let read = entry.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            target.write_all(&buffer[..read])?;
        }
        out.push((staging.entry_path(out.len()), relative));
    }
    Ok(())
}

struct Staging {
    directory: tempfile::TempDir,
}

impl Staging {
    fn create(cache: &Path) -> std::io::Result<Self> {
        Ok(Self {
            directory: tempfile::Builder::new().prefix("zip-").tempdir_in(cache)?,
        })
    }

    fn entry_path(&self, index: usize) -> PathBuf {
        self.directory.path().join(format!("{index}.entry"))
    }
}

async fn build_download(
    state: &CoreRuntime,
    items: Vec<(PathBuf, String)>,
    name: &str,
    staging: Option<Staging>,
    permit: tokio::sync::OwnedSemaphorePermit,
) -> Result<Download, Error> {
    for (path, _) in items.iter().filter(|_| staging.is_none()) {
        plain_rs::services::files::links::authorize_path(state, &path.to_string_lossy())
            .map_err(|error| Error::Denied(error.to_string()))?;
    }
    let _staging = staging;
    let reader = state
        .archives
        .build(items, permit, state.stop.clone())
        .await
        .map_err(Error::Invalid)?;
    Ok(Download {
        reader,
        name: name.to_owned(),
    })
}
pub async fn directory(state: &CoreRuntime, id: &str) -> Result<Download, Error> {
    let permit = state.archives.acquire().ok_or(Error::Busy)?;
    if id.is_empty() {
        return Err(Error::Invalid("".into()));
    }
    // Share links stay reachable without desktop access, exactly like before.
    if let Ok((plain_rs::files::resource_link::Scope::Share(sid), _)) =
        plain_rs::files::resource_link::parse(id)
    {
        let shares = plain_rs::files::shares::Service::new(state.db.clone(), state.prefs.clone());
        let path = match shares.resolve_file(&sid, id) {
            Ok(Some(path)) => PathBuf::from(path),
            _ => return Err(Error::Denied("".into())),
        };
        let entries = match shares.archive(&sid, id) {
            Ok(entries) => entries,
            Err(error) => return Err(Error::Invalid(error.to_string())),
        };
        let items: Vec<(PathBuf, String)> = entries
            .into_iter()
            .map(|entry| (PathBuf::from(entry.real_path), entry.relative_path))
            .collect();
        let name = format!(
            "{}.zip",
            path.file_name()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_default()
        );
        return build_download(state, items, &name, None, permit).await;
    }
    if !desktop_access_allowed(state) {
        return Err(Error::Denied("".into()));
    }
    let file = match plain_rs::services::files::links::resolve(state, id).await {
        Ok(file) => file,
        Err(error) => return Err(Error::Denied(error.to_string())),
    };
    let (path, json_name) = (file.path, file.name);
    if path.is_empty() {
        return Err(Error::Invalid("".into()));
    }
    if let Some((archive, inner)) = split_zip_path(&path) {
        let staging = match Staging::create(state.cache.root()) {
            Ok(staging) => staging,
            Err(_) => {
                return Err(Error::Failed("Failed to stage the archive".into()));
            }
        };
        let mut items = Vec::new();
        if collect_archive_entries(&archive, &inner, &staging, &mut items).is_err() {
            return Err(Error::Invalid("Failed to read the archive".into()));
        }
        let folder = if inner.is_empty() {
            archive
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_default()
        } else {
            inner.rsplit('/').next().unwrap_or_default().to_owned()
        };
        let name = if json_name.is_empty() {
            format!("{folder}.zip")
        } else {
            json_name
        };
        return build_download(state, items, &name, Some(staging), permit).await;
    }
    let root = PathBuf::from(&path);
    if !root.is_dir() {
        return Err(Error::Missing);
    }
    let mut items = Vec::new();
    if walk_directory(&root, "", &mut items).is_err() {
        return Err(Error::Invalid("Failed to read the directory".into()));
    }
    let folder = root
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();
    let name = if json_name.is_empty() {
        format!("{folder}.zip")
    } else {
        json_name
    };
    build_download(state, items, &name, None, permit).await
}

/// Resolve attachment paths against the shared data root.
fn stored_path(directory: &Path, stored: &str) -> PathBuf {
    let path = Path::new(stored);
    if stored.is_empty() || path.is_absolute() {
        path.to_path_buf()
    } else {
        directory.join(path)
    }
}

fn app_file_entries(state: &CoreRuntime, query: &str) -> anyhow::Result<Vec<(PathBuf, String)>> {
    let names =
        plain_rs::chat::attachments::app_file_store::file_name_map(&state.db.get_all_chats());
    let files = match query.strip_prefix("ids:") {
        Some(ids) => {
            let mut rows = Vec::new();
            for id in ids.split(',').filter(|id| !id.is_empty()) {
                if let Some(row) = state
                    .db
                    .app_file_get(id.split('.').next().unwrap_or_default())?
                {
                    rows.push(row);
                }
            }
            rows
        }
        None => state.db.app_file_items(0, i32::MAX, query)?,
    };
    Ok(files
        .into_iter()
        .map(|file| {
            let entry_name =
                plain_rs::chat::attachments::app_file_store::display_name(&file, &names);
            (stored_path(&state.directory, &file.real_path), entry_name)
        })
        .collect())
}

/// Media rows have no separate archive name, so each entry keeps its own file
/// name and the archive builder resolves it from the path.
async fn media_entries(
    state: &CoreRuntime,
    data_type: &str,
    query: &str,
) -> anyhow::Result<Vec<(PathBuf, String)>> {
    let kind = match data_type {
        "AUDIO" => DataType::Audio,
        "VIDEO" => DataType::Video,
        "IMAGE" => DataType::Image,
        "DOC" => DataType::Doc,
        other => anyhow::bail!("unsupported media catalog type: {other}"),
    };
    let rows =
        plain_rs::services::files::catalog::MediaCatalog::new(state.db.clone(), state.host.clone())
            .rows(
                kind,
                query,
                plain_rs::prefs::settings::FileSortBy::DateDesc,
                i32::MAX as u32,
                0,
            )
            .await
            .map_err(anyhow::Error::msg)?;
    Ok(rows
        .iter()
        .filter_map(|row| Some((PathBuf::from(row["path"].as_str()?), String::new())))
        .collect())
}

async fn package_entries(
    state: &CoreRuntime,
    query: &str,
) -> anyhow::Result<Vec<(PathBuf, String)>> {
    plain_rs::services::capabilities::require(&state.host, "PACKAGES").await?;
    let facts = state
        .host
        .call("systemPackageFacts", json!({}))
        .await
        .map_err(anyhow::Error::msg)?;
    let facts: Vec<Value> = serde_json::from_value(facts)?;
    let items = plain_rs::services::packages::query::packages(&state.db, facts, query, "NAME_ASC")?;
    Ok(items
        .iter()
        .filter_map(|item| {
            let name = item["name"].as_str().unwrap_or_default().replace(' ', "");
            let id = item["id"].as_str().unwrap_or_default();
            Some((
                PathBuf::from(item["path"].as_str()?),
                format!("{name}-{id}.apk"),
            ))
        })
        .collect())
}

#[derive(Deserialize)]
struct DownloadFileItem {
    path: String,
    #[serde(default)]
    name: String,
}

/// Consumes the selection `setTempValue` parked under `key`.
fn temp_file_entries(state: &CoreRuntime, key: &str) -> Vec<(PathBuf, String)> {
    let Some(value) = state.temp_values.take(key) else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<DownloadFileItem>>(&value)
        .map(|items| {
            items
                .into_iter()
                .map(|item| (PathBuf::from(item.path), item.name))
                .collect()
        })
        .unwrap_or_default()
}

async fn zip_items(
    state: &CoreRuntime,
    request: &ZipFilesRequest,
) -> anyhow::Result<Vec<(PathBuf, String)>> {
    let data_type: DataType = serde_json::from_value(Value::String(request.r#type.clone()))?;
    match data_type {
        DataType::AppFile => app_file_entries(state, &request.query),
        DataType::Audio | DataType::Video | DataType::Image | DataType::Doc => {
            let name = match data_type {
                DataType::Audio => "AUDIO",
                DataType::Video => "VIDEO",
                DataType::Doc => "DOC",
                _ => "IMAGE",
            };
            media_entries(state, name, &request.query).await
        }
        DataType::Package => package_entries(state, &request.query).await,
        DataType::File => Ok(temp_file_entries(state, &request.id)),
        _ => anyhow::bail!("unsupported archive type"),
    }
}

pub async fn files(state: &CoreRuntime, id: &str) -> Result<Download, Error> {
    let permit = state.archives.acquire().ok_or(Error::Busy)?;
    if !desktop_access_allowed(state) {
        return Err(Error::Denied("".into()));
    }
    let Some(plaintext) = decrypt_url_id(state, id) else {
        return Err(Error::Invalid("File is expired or does not exist.".into()));
    };
    let request: ZipFilesRequest = match serde_json::from_str(&plaintext) {
        Ok(request) => request,
        Err(error) => return Err(Error::Invalid(error.to_string())),
    };
    if request.r#type.is_empty() {
        return Err(Error::Invalid("".into()));
    }
    // The host answers only the platform lookups it owns; attachment rows and
    // the parked selection are already in this process.
    let items: Vec<(PathBuf, String)> = match zip_items(state, &request).await {
        Ok(items) => items,
        Err(error) => return Err(Error::Invalid(error.to_string())),
    }
    .into_iter()
    .filter(|(path, _)| path.is_file())
    .collect();
    if items.is_empty() {
        return Err(Error::Missing);
    }
    let name = if request.name.is_empty() {
        "download.zip".to_owned()
    } else {
        request.name
    };
    build_download(state, items, &name, None, permit).await
}

#[cfg(test)]
#[path = "../../../tests/unit/services/files/archive_downloads.rs"]
mod tests;
