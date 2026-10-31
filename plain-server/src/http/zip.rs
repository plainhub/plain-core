//! `GET /zip/dir` and `GET /zip/files` — archive downloads for the web UI.
//!
//! Three directory shapes are supported, matching plain-app's `ZipRoutes.kt`:
//! a share link (`?sid=`, encrypted with the share's own url token), a virtual
//! path inside an existing archive (`….zip!zip!/inner/path`), and a real
//! directory. `/zip/files` zips a search result instead.

use crate::server::ServerState;
use axum::{
    body::Body,
    extract::{Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use plain_rs::enums::DataType;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};

const ZIP_SEPARATOR: &str = "!zip!/";
const MAX_ENTRIES: usize = 100_000;

#[derive(Deserialize)]
pub(crate) struct Params {
    #[serde(default)]
    id: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct ZipFilesRequest {
    r#type: String,
    query: String,
    id: String,
    name: String,
}

fn desktop_access_allowed(state: &ServerState) -> bool {
    state.prefs.get_or("desktop_access", true)
}

fn plain(status: StatusCode, body: impl Into<String>) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        body.into(),
    )
        .into_response()
}

/// plain-app percent-encodes the download name; the desktop route does the
/// same before writing the `Content-Disposition`.
fn url_escape(value: &str) -> String {
    crate::http::file_response::escape(value)
}

/// Decrypt a urlToken-protected `id` into its plaintext payload.
fn decrypt_url_id(state: &ServerState, id: &str) -> Option<String> {
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

async fn stream_archive(
    state: &ServerState,
    items: Vec<(PathBuf, String)>,
    name: &str,
    staging: Option<Staging>,
    permit: tokio::sync::OwnedSemaphorePermit,
) -> Response {
    for (path, _) in items.iter().filter(|_| staging.is_none()) {
        if let Err(error) =
            plain_rs::services::files::links::authorize_path(state, &path.to_string_lossy())
        {
            return plain(StatusCode::FORBIDDEN, error.to_string());
        }
    }

    let _staging = staging;
    let reader = match state
        .archives
        .build(items, permit, state.stop.clone())
        .await
    {
        Ok(reader) => reader,
        Err(error) => return plain(StatusCode::BAD_REQUEST, error),
    };
    let encoded = url_escape(name);
    let disposition = format!("attachment;filename=\"{encoded}\";filename*=utf-8''{encoded}");
    let length = reader.size;
    let stream = futures_util::stream::try_unfold(reader, |mut reader| async move {
        reader
            .read()
            .await
            .map(|chunk| chunk.map(|bytes| (axum::body::Bytes::from(bytes), reader)))
            .map_err(std::io::Error::other)
    });
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/zip")
        .header(header::CONTENT_DISPOSITION, disposition)
        .header(header::CONTENT_LENGTH, length)
        .body(Body::from_stream(stream))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}
async fn zip_dir(
    state: &ServerState,
    params: &Params,
    permit: tokio::sync::OwnedSemaphorePermit,
) -> Response {
    if params.id.is_empty() {
        return plain(StatusCode::BAD_REQUEST, "");
    }
    // Share links stay reachable without desktop access, exactly like before.
    if let Ok((plain_rs::files::resource_link::Scope::Share(sid), _)) =
        plain_rs::files::resource_link::parse(&params.id)
    {
        let shares = plain_rs::files::shares::Service::new(state.db.clone(), state.prefs.clone());
        let path = match shares.resolve_file(&sid, &params.id) {
            Ok(Some(path)) => PathBuf::from(path),
            _ => return plain(StatusCode::FORBIDDEN, ""),
        };
        let entries = match shares.archive(&sid, &params.id) {
            Ok(entries) => entries,
            Err(error) => return plain(StatusCode::BAD_REQUEST, error.to_string()),
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
        return stream_archive(state, items, &name, None, permit).await;
    }
    if !desktop_access_allowed(state) {
        return plain(StatusCode::FORBIDDEN, "");
    }
    let file = match plain_rs::services::files::links::resolve(state, &params.id).await {
        Ok(file) => file,
        Err(error) => return plain(StatusCode::FORBIDDEN, error.to_string()),
    };
    let (path, json_name) = (file.path, file.name);
    if path.is_empty() {
        return plain(StatusCode::BAD_REQUEST, "");
    }
    if let Some((archive, inner)) = split_zip_path(&path) {
        let staging = match Staging::create(state.cache.root()) {
            Ok(staging) => staging,
            Err(_) => {
                return plain(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to stage the archive",
                );
            }
        };
        let mut items = Vec::new();
        if collect_archive_entries(&archive, &inner, &staging, &mut items).is_err() {
            return plain(StatusCode::BAD_REQUEST, "Failed to read the archive");
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
        return stream_archive(state, items, &name, Some(staging), permit).await;
    }
    let root = PathBuf::from(&path);
    if !root.is_dir() {
        return plain(StatusCode::NOT_FOUND, "");
    }
    let mut items = Vec::new();
    if walk_directory(&root, "", &mut items).is_err() {
        return plain(StatusCode::BAD_REQUEST, "Failed to read the directory");
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
    stream_archive(state, items, &name, None, permit).await
}

/// Rows store the relative portion of an attachment path, but a row written
/// before that convention may still hold an absolute one.
fn stored_path(directory: &Path, stored: &str) -> PathBuf {
    let path = Path::new(stored);
    if stored.is_empty() || path.is_absolute() {
        path.to_path_buf()
    } else {
        directory.join(path)
    }
}

fn app_file_entries(state: &ServerState, query: &str) -> anyhow::Result<Vec<(PathBuf, String)>> {
    let names =
        plain_rs::chat::attachments::app_file_store::file_name_map(&state.db.get_all_chats());
    let files = match query.strip_prefix("ids:") {
        Some(ids) => ids
            .split(',')
            .filter(|id| !id.is_empty())
            .filter_map(|id| {
                state
                    .db
                    .app_file_get(id.split('.').next().unwrap_or_default())
                    .ok()
                    .flatten()
            })
            .collect(),
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
    state: &ServerState,
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
    state: &ServerState,
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
fn temp_file_entries(state: &ServerState, key: &str) -> Vec<(PathBuf, String)> {
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
    state: &ServerState,
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

async fn zip_files(
    state: &ServerState,
    params: &Params,
    permit: tokio::sync::OwnedSemaphorePermit,
) -> Response {
    if !desktop_access_allowed(state) {
        return plain(StatusCode::FORBIDDEN, "");
    }
    let Some(plaintext) = decrypt_url_id(state, &params.id) else {
        return plain(
            StatusCode::BAD_REQUEST,
            "File is expired or does not exist.",
        );
    };
    let request: ZipFilesRequest = match serde_json::from_str(&plaintext) {
        Ok(request) => request,
        Err(error) => return plain(StatusCode::BAD_REQUEST, error.to_string()),
    };
    if request.r#type.is_empty() {
        return plain(StatusCode::BAD_REQUEST, "");
    }
    // The host answers only the platform lookups it owns; attachment rows and
    // the parked selection are already in this process.
    let items: Vec<(PathBuf, String)> = match zip_items(state, &request).await {
        Ok(items) => items,
        Err(error) => return plain(StatusCode::BAD_REQUEST, error.to_string()),
    }
    .into_iter()
    .filter(|(path, _)| path.is_file())
    .collect();
    if items.is_empty() {
        return plain(StatusCode::NOT_FOUND, "");
    }
    let name = if request.name.is_empty() {
        "download.zip".to_owned()
    } else {
        request.name
    };
    stream_archive(state, items, &name, None, permit).await
}

pub(crate) async fn dir(
    State(state): State<ServerState>,
    Query(params): Query<Params>,
) -> Response {
    let Some(permit) = state.archives.acquire() else {
        return plain(StatusCode::TOO_MANY_REQUESTS, "");
    };
    zip_dir(&state, &params, permit).await
}

pub(crate) async fn files(
    State(state): State<ServerState>,
    Query(params): Query<Params>,
) -> Response {
    let Some(permit) = state.archives.acquire() else {
        return plain(StatusCode::TOO_MANY_REQUESTS, "");
    };
    zip_files(&state, &params, permit).await
}

#[cfg(test)]
#[path = "../../tests/unit/http/zip.rs"]
mod tests;
