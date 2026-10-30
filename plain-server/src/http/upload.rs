//! `POST /upload` and `POST /upload_chunk` for the public web UI.
//!
//! The `info` part is base64 of the XChaCha20-Poly1305 blob encrypted with the
//! session token that Rust itself issued and stored in the `sessions` row, so
//! no host round trip is needed to authenticate a part.
//!
//! Behaviour mirrors plain-app's `UploadRoutes.kt`: file bodies stream to a
//! temp file, the received size is verified against `info.size`, and the final
//! placement is an atomic rename so a cancelled upload never leaves a partial
//! file at the destination.
//!
//! A chunk that declares `size > 0` is written through the shared
//! `UploadChunks` writer, which owns the temp file and the atomic placement. A
//! chunk with no declared size keeps the legacy streaming path, because the
//! shared writer cannot finish an unknown-length chunk.

use crate::assembly::server::ServerState;
use crate::services::files::upload_chunks::MAX_UPLOAD_WRITE_BYTES;
use axum::{
    extract::{FromRequest, Multipart, Request, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct UploadInfo {
    dir: String,
    replace: bool,
    is_app_file: bool,
    size: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct ChunkInfo {
    file_id: String,
    index: i64,
    size: i64,
}

fn plain(status: StatusCode, body: impl Into<String>) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        body.into(),
    )
        .into_response()
}

fn desktop_access_allowed(state: &ServerState) -> bool {
    state.prefs.get_or("desktop_access", true)
}

/// The session token Rust issued, straight from the `sessions` row.
async fn session_key(state: &ServerState, headers: &HeaderMap) -> anyhow::Result<Vec<u8>> {
    let client_id = headers
        .get("c-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    anyhow::ensure!(!client_id.is_empty(), "c-id header is missing");
    if state.authenticated(headers) {
        if let Some(key) = crate::services::sessions::local_key(state, client_id) {
            return Ok(key);
        }
    }
    let row = state
        .db
        .session_get(client_id)
        .map_err(|error| anyhow::Error::msg(error.to_string()))?
        .ok_or_else(|| anyhow::anyhow!("unknown client"))?;
    let key = plain_rs::utils::base64::base64_decode(&row.token);
    anyhow::ensure!(key.len() == 32, "session token is not a 32-byte key");
    Ok(key)
}

async fn read_info<T: for<'de> Deserialize<'de>>(
    state: &ServerState,
    client: &str,
    key: &[u8],
    field: axum::extract::multipart::Field<'_>,
) -> anyhow::Result<T> {
    let bytes = field
        .bytes()
        .await
        .map_err(|error| anyhow::Error::msg(error.to_string()))?;
    let plaintext = plain_rs::crypto::xchacha_decrypt_raw(key, &bytes)
        .ok_or_else(|| anyhow::anyhow!("Unauthorized"))?;
    let plaintext = std::str::from_utf8(&plaintext)?;
    let payload = state
        .main_replay
        .admit(client, plaintext, chrono::Utc::now().timestamp_millis())
        .map_err(|_| anyhow::anyhow!("invalid or replayed upload metadata"))?;
    Ok(serde_json::from_str(payload)?)
}

/// Streams one multipart part into `destination`, returning the byte count.
async fn stream_to_file(
    field: &mut axum::extract::multipart::Field<'_>,
    destination: &Path,
) -> anyhow::Result<u64> {
    if let Some(parent) = destination.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| anyhow::Error::msg(error.to_string()))?;
    }
    let mut file = tokio::fs::File::create(destination)
        .await
        .map_err(|error| anyhow::Error::msg(error.to_string()))?;
    let mut written = 0u64;
    loop {
        let chunk = field
            .chunk()
            .await
            .map_err(|error| anyhow::Error::msg(error.to_string()))?;
        let Some(chunk) = chunk else { break };
        file.write_all(&chunk)
            .await
            .map_err(|error| anyhow::Error::msg(error.to_string()))?;
        written += chunk.len() as u64;
    }
    file.flush()
        .await
        .map_err(|error| anyhow::Error::msg(error.to_string()))?;
    Ok(written)
}

struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn temp_name(prefix: &str) -> String {
    format!(".{prefix}_{}", uuid::Uuid::new_v4())
}

/// A declared-length chunk goes through the shared writer, whose `Drop`
/// unlinks the temp file on every early return.
async fn store_chunk(
    state: &ServerState,
    field: &mut axum::extract::multipart::Field<'_>,
    info: &ChunkInfo,
) -> Result<i64, String> {
    let declared = info.size as u64;
    let mut chunk = state
        .upload_chunks
        .open_chunk(&info.file_id, info.index, declared, state.stop.clone())
        .await?;
    let mut written = 0u64;
    loop {
        let bytes = match field.chunk().await {
            Ok(Some(bytes)) => bytes,
            Ok(None) => break,
            Err(error) => return Err(error.to_string()),
        };
        for slice in bytes.chunks(MAX_UPLOAD_WRITE_BYTES) {
            chunk.write(slice).await?;
            written += slice.len() as u64;
        }
    }
    match chunk.finish().await {
        Ok(saved) => Ok(saved as i64),
        // Short writes keep the wording the HTTP clients already match on.
        Err(_) if written != declared => Err(format!(
            "Chunk {} final size mismatch: expected {}, saved {written}",
            info.index, info.size
        )),
        Err(error) => Err(error),
    }
}

pub(crate) async fn upload_tmp_dir(state: &ServerState) -> anyhow::Result<String> {
    Ok(state
        .upload_chunks
        .directory()
        .to_string_lossy()
        .into_owned())
}

pub(crate) async fn upload(State(state): State<ServerState>, request: Request) -> Response {
    if !desktop_access_allowed(&state) {
        return plain(StatusCode::FORBIDDEN, "forbidden");
    }
    let key = match session_key(&state, request.headers()).await {
        Ok(key) => key,
        Err(_) => return plain(StatusCode::UNAUTHORIZED, "unauthorized"),
    };
    let client = request
        .headers()
        .get("c-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let mut multipart = match Multipart::from_request(request, &()).await {
        Ok(multipart) => multipart,
        Err(_) => return plain(StatusCode::BAD_REQUEST, "invalid multipart request"),
    };
    let mut info: Option<UploadInfo> = None;
    let mut saved: Option<String> = None;
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(_) => return plain(StatusCode::BAD_REQUEST, "invalid multipart body"),
        };
        let name = field.name().unwrap_or_default().to_owned();
        let mut field = field;
        match name.as_str() {
            "info" => match read_info(&state, &client, &key, field).await {
                Ok(value) => info = Some(value),
                Err(error) if error.to_string() == "Unauthorized" => {
                    return plain(StatusCode::UNAUTHORIZED, "unauthorized");
                }
                Err(error) => return plain(StatusCode::BAD_REQUEST, error.to_string()),
            },
            "file" => {
                let Some(current) = info.clone() else {
                    return plain(StatusCode::BAD_REQUEST, "info part must precede file part");
                };
                let file_name = field
                    .file_name()
                    .unwrap_or_default()
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or_default()
                    .to_owned();
                if file_name.is_empty()
                    || matches!(file_name.as_str(), "." | "..")
                    || current.dir.is_empty()
                {
                    return plain(StatusCode::BAD_REQUEST, "dir or fileName is empty");
                }
                let content_type = field.content_type().unwrap_or_default().to_owned();
                let destination_dir = PathBuf::from(&current.dir);
                if current.is_app_file {
                    match store_app_file(
                        &state,
                        &mut field,
                        &file_name,
                        &content_type,
                        current.size,
                    )
                    .await
                    {
                        Ok(suffix) => saved = Some(suffix),
                        Err(error) => return plain(StatusCode::BAD_REQUEST, error.to_string()),
                    }
                    continue;
                }
                if !destination_dir.is_absolute() {
                    return plain(
                        StatusCode::BAD_REQUEST,
                        "absolute upload directory required",
                    );
                }
                if let Err(error) = state
                    .file_platform
                    .authorize(
                        destination_dir
                            .join(&file_name)
                            .to_str()
                            .unwrap_or_default(),
                        true,
                    )
                    .await
                {
                    return plain(StatusCode::BAD_REQUEST, error.to_string());
                }
                let original = destination_dir.join(&file_name);
                let mut destination = if current.replace {
                    original.clone()
                } else {
                    match plain_rs::utils::unique_path::unique_sibling(&original) {
                        Ok(path) => path,
                        Err(error) => return plain(StatusCode::BAD_REQUEST, error.to_string()),
                    }
                };
                let temp = destination_dir.join(temp_name("upload_tmp"));
                let _temporary = Temporary(temp.clone());
                let written = match stream_to_file(&mut field, &temp).await {
                    Ok(written) => written,
                    Err(error) => {
                        let _ = tokio::fs::remove_file(&temp).await;
                        return plain(StatusCode::BAD_REQUEST, error.to_string());
                    }
                };
                if current.size > 0 && written != current.size as u64 {
                    let _ = tokio::fs::remove_file(&temp).await;
                    return plain(
                        StatusCode::BAD_REQUEST,
                        format!("Size mismatch: expected {}, got {written}", current.size),
                    );
                }
                let commit = if current.replace {
                    tokio::fs::rename(&temp, &destination).await
                } else {
                    loop {
                        let source = temp.clone();
                        let target = destination.clone();
                        let result = tokio::task::spawn_blocking(move || {
                            plain_rs::files::filesystem::rename::no_replace(&source, &target)
                        })
                        .await;
                        let result = match result {
                            Ok(result) => result,
                            Err(error) => break Err(std::io::Error::other(error)),
                        };
                        if result
                            .as_ref()
                            .is_err_and(|error| error.kind() == std::io::ErrorKind::AlreadyExists)
                        {
                            destination =
                                match plain_rs::utils::unique_path::unique_sibling(&original) {
                                    Ok(path) => path,
                                    Err(error) => break Err(error),
                                };
                        } else {
                            break result;
                        }
                    }
                };
                if let Err(error) = commit {
                    let _ = tokio::fs::remove_file(&temp).await;
                    return plain(
                        StatusCode::BAD_REQUEST,
                        format!("Failed to move uploaded file into place: {error}"),
                    );
                }
                if let Err(error) = state
                    .file_platform
                    .scan(&[destination.to_string_lossy().into_owned()])
                    .await
                {
                    return plain(
                        StatusCode::BAD_REQUEST,
                        format!(
                            "Uploaded file saved at {} but scan failed: {error}",
                            destination.display()
                        ),
                    );
                }
                saved = Some(
                    destination
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                );
            }
            _ => {
                let _ = field.bytes().await;
            }
        }
    }
    match saved {
        Some(name) => plain(StatusCode::CREATED, name),
        None => plain(StatusCode::BAD_REQUEST, "no file uploaded"),
    }
}

async fn store_app_file(
    state: &ServerState,
    field: &mut axum::extract::multipart::Field<'_>,
    name: &str,
    content_type: &str,
    expected: i64,
) -> anyhow::Result<String> {
    let directory = state.directory.clone();
    let temp = directory.join(temp_name("chat_upload"));
    let _temporary = Temporary(temp.clone());
    let written = match stream_to_file(field, &temp).await {
        Ok(written) => written,
        Err(error) => {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(error);
        }
    };
    if expected > 0 && written != expected as u64 {
        let _ = tokio::fs::remove_file(&temp).await;
        return Err(anyhow::anyhow!(
            "Size mismatch: expected {expected}, got {written}"
        ));
    }
    let store = plain_rs::files::app_files::FileStore::new(state.db.clone(), directory);
    match store
        .import(temp.clone(), name.to_owned(), content_type.to_owned(), true)
        .await
    {
        // The client keys the stored file by `{hash}.{ext}`, so the suffix must
        // come from the record's real path (a dedup hit can carry a different
        // extension than a fresh guess would produce).
        Ok(record) => Path::new(&record.real_path)
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::to_owned)
            .ok_or_else(|| anyhow::anyhow!("Failed to import app file")),
        Err(error) => {
            let _ = tokio::fs::remove_file(&temp).await;
            Err(anyhow::anyhow!(error))
        }
    }
}

pub(crate) async fn upload_chunk(State(state): State<ServerState>, request: Request) -> Response {
    if !desktop_access_allowed(&state) {
        return plain(StatusCode::FORBIDDEN, "forbidden");
    }
    let key = match session_key(&state, request.headers()).await {
        Ok(key) => key,
        Err(_) => return plain(StatusCode::UNAUTHORIZED, "unauthorized"),
    };
    let client = request
        .headers()
        .get("c-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let mut multipart = match Multipart::from_request(request, &()).await {
        Ok(multipart) => multipart,
        Err(_) => return plain(StatusCode::BAD_REQUEST, "invalid multipart request"),
    };
    let mut info: Option<ChunkInfo> = None;
    let mut saved_size = 0i64;
    let mut failure: Option<Response> = None;
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(_) => {
                failure = Some(plain(StatusCode::BAD_REQUEST, "invalid multipart body"));
                break;
            }
        };
        let name = field.name().unwrap_or_default().to_owned();
        let mut field = field;
        match name.as_str() {
            "info" => match read_info(&state, &client, &key, field).await {
                Ok(value) => info = Some(value),
                Err(error) if error.to_string() == "Unauthorized" => {
                    failure = Some(plain(StatusCode::UNAUTHORIZED, "unauthorized"));
                    break;
                }
                Err(error) => {
                    failure = Some(plain(StatusCode::BAD_REQUEST, error.to_string()));
                    break;
                }
            },
            "file" => {
                let Some(current) = info.clone() else {
                    failure = Some(plain(
                        StatusCode::BAD_REQUEST,
                        "info part must precede file part",
                    ));
                    break;
                };
                if current.file_id.is_empty() || current.index < 0 || current.size < 0 {
                    failure = Some(plain(
                        StatusCode::BAD_REQUEST,
                        "fileId or index is missing or invalid",
                    ));
                    break;
                }
                if current.size > 0 {
                    match store_chunk(&state, &mut field, &current).await {
                        Ok(saved) => saved_size = saved,
                        Err(error) => {
                            failure = Some(plain(StatusCode::BAD_REQUEST, error));
                            break;
                        }
                    }
                    continue;
                }
                let base = match upload_tmp_dir(&state).await {
                    Ok(base) => match plain_rs::files::uploads::chunk_path(
                        Path::new(&base),
                        &current.file_id,
                        current.index,
                    ) {
                        Ok(path) => path,
                        Err(error) => {
                            failure = Some(plain(StatusCode::BAD_REQUEST, error.to_string()));
                            break;
                        }
                    },
                    Err(error) => {
                        failure = Some(plain(StatusCode::BAD_REQUEST, error.to_string()));
                        break;
                    }
                };
                let chunk = base;
                let temp = chunk
                    .parent()
                    .unwrap()
                    .join(temp_name(&format!("tmp_chunk_{}", current.index)));
                let _temporary = Temporary(temp.clone());
                match stream_to_file(&mut field, &temp).await {
                    Ok(_) => {
                        let _ = tokio::fs::remove_file(&chunk).await;
                        if tokio::fs::rename(&temp, &chunk).await.is_err() {
                            // Some Android filesystems report a failed rename
                            // that still moved the file; trust the size of the
                            // final path, exactly like the Kotlin route.
                            let final_size = tokio::fs::metadata(&chunk)
                                .await
                                .map(|meta| meta.len() as i64)
                                .unwrap_or(-1);
                            if final_size < 0 {
                                let _ = tokio::fs::remove_file(&temp).await;
                                failure = Some(plain(
                                    StatusCode::BAD_REQUEST,
                                    format!(
                                        "Failed to save chunk {}: rename failed and source file is missing",
                                        current.index
                                    ),
                                ));
                                break;
                            }
                            saved_size = final_size;
                        } else {
                            saved_size = match tokio::fs::metadata(&chunk).await {
                                Ok(meta) => meta.len() as i64,
                                Err(_) => 0,
                            };
                        }
                    }
                    Err(error) => {
                        let _ = tokio::fs::remove_file(&temp).await;
                        failure = Some(plain(StatusCode::BAD_REQUEST, error.to_string()));
                        break;
                    }
                }
            }
            _ => {
                let _ = field.bytes().await;
            }
        }
    }
    if let Some(response) = failure {
        return response;
    }
    let index = info.map(|value| value.index).unwrap_or(0);
    if saved_size > 0 {
        plain(StatusCode::CREATED, format!("{index}:{saved_size}"))
    } else {
        plain(StatusCode::BAD_REQUEST, "chunk upload failed")
    }
}

#[cfg(test)]
#[path = "../../tests/unit/http/upload.rs"]
mod tests;
