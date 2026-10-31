use plain_rs::runtime::Runtime as CoreRuntime;
use plain_rs::services::files::links;
use plain_rs::services::files::previews;
use serde_json::json;
#[cfg(feature = "runtime-media")]
use std::path::Path;
use std::path::PathBuf;

pub struct Options {
    pub download: bool,
    pub offset: Option<i64>,
    pub length: Option<usize>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub center_crop: bool,
    #[cfg(feature = "runtime-media")]
    pub pdf: bool,
    pub probe: bool,
    pub transcode: bool,
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub starts_view: bool,
}

pub enum Content {
    File {
        path: PathBuf,
        mime: String,
        name: String,
        download: bool,
    },
    Temporary {
        file: previews::Temporary,
        mime: &'static str,
        name: Option<String>,
    },
    Resource {
        path: String,
        name: Option<String>,
        download: bool,
    },
    Bytes(Vec<u8>),
    Codec(String),
    Thumbnail {
        bytes: Result<Option<Vec<u8>>, String>,
        name: String,
    },
    NotFound,
    Directory,
    SliceTooLarge,
    TranscodingUnavailable,
}

pub async fn read(
    state: &CoreRuntime,
    file: links::File,
    options: Options,
) -> anyhow::Result<Content> {
    let links::File {
        path,
        media_id,
        name,
    } = file;
    let filename = if name.is_empty() {
        path.rsplit('/').next().unwrap_or_default()
    } else {
        &name
    };
    if path.contains("!zip!/") {
        let cached = state
            .archives
            .preview(&state.file_platform, path.clone())
            .await
            .map_err(anyhow::Error::msg)?;
        let filename = if name.is_empty() {
            path.trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or_default()
        } else {
            &name
        };
        return Ok(Content::File {
            mime: plain_rs::utils::mime::mime_from_ext(&cached.to_string_lossy()).to_owned(),
            path: cached,
            name: filename.to_owned(),
            download: options.download,
        });
    }
    if let (Some(offset), Some(length)) = (options.offset, options.length) {
        if length > 0 && !path.starts_with("content://") && !path.starts_with("pkgicon://") {
            return read_slice(&path, offset, length).await;
        }
    }
    if path.starts_with("content://") {
        if let Some(file) = previews::convert(state, previews::Conversion::Video3gp, &path).await? {
            return Ok(Content::Temporary {
                file,
                mime: "video/mp4",
                name: None,
            });
        }
        return Ok(Content::Resource {
            name: options.download.then(|| filename.to_owned()),
            path,
            download: options.download,
        });
    }
    if let Some(package) = path.strip_prefix("pkgicon://") {
        return Ok(
            match previews::convert(state, previews::Conversion::PackageIcon, package).await? {
                Some(file) => Content::Temporary {
                    file,
                    mime: "application/octet-stream",
                    name: None,
                },
                None => Content::NotFound,
            },
        );
    }
    let meta = match tokio::fs::metadata(&path).await {
        Ok(meta) => meta,
        Err(_) => return Ok(Content::NotFound),
    };
    if meta.is_dir() {
        return Ok(Content::Directory);
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    if options.starts_view && options.width.is_none_or(|width| width > 200) {
        if let Err(error) = plain_rs::recent_files::add(state.store.get()?.as_ref(), &path) {
            log::warn!("Recent file update failed: {error}");
        }
    }
    #[cfg(feature = "runtime-media")]
    if !options.download && options.pdf {
        let modified = meta
            .modified()?
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64;
        let pdf = plain_rs::services::files::docs_preview::get_or_create_pdf_preview(
            state.cache.root(),
            &path,
            modified,
            meta.len() as i64,
        )
        .await?;
        let name = Path::new(if name.is_empty() { &path } else { &name })
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("document");
        return Ok(Content::File {
            path: pdf,
            mime: "application/pdf".into(),
            name: format!("{name}.pdf"),
            download: false,
        });
    }
    let mime = plain_rs::utils::mime::mime_from_ext(&path);
    if options.probe {
        let codec =
            plain_rs::services::files::resources::read(&state.host, "probe", &path, json!({}))
                .await
                .map_err(anyhow::Error::msg)?;
        return Ok(Content::Codec(
            codec
                .as_str()
                .unwrap_or_default()
                .chars()
                .filter(|ch| ch.is_alphanumeric())
                .collect(),
        ));
    }
    if !options.download {
        let animated = if mime.starts_with("image/") {
            plain_rs::services::files::resources::read(
                &state.host,
                "animated",
                &path,
                json!({"fileName":plain_rs::utils::query::url_encode(filename)}),
            )
            .await
            .map_err(anyhow::Error::msg)?
            .as_bool()
                == Some(true)
        } else {
            false
        };
        if !animated {
            if let (Some(width), Some(height)) = (options.width, options.height) {
                let bytes = state
                    .thumbnails
                    .get(
                        state,
                        plain_rs::services::images::thumbnails::Request {
                            path: path.clone(),
                            width,
                            height,
                            center_crop: options.center_crop,
                            file_id: media_id,
                            file_name: plain_rs::utils::query::url_encode(filename),
                        },
                    )
                    .await;
                return Ok(Content::Thumbnail {
                    bytes,
                    name: filename.to_owned(),
                });
            }
            if let Some(file) = previews::convert(state, previews::Conversion::Png, &path).await? {
                return Ok(Content::Temporary {
                    file,
                    mime: "image/png",
                    name: Some(filename.to_owned()),
                });
            }
            if mime == "video/mp4" {
                if options.transcode {
                    let codec = plain_rs::services::files::resources::read(
                        &state.host,
                        "probe",
                        &path,
                        json!({}),
                    )
                    .await
                    .map_err(anyhow::Error::msg)?;
                    if matches!(codec.as_str(), Some("hvc1" | "hev1")) {
                        return Ok(
                            match state
                                .browser
                                .convert(
                                    &path,
                                    plain_rs::services::files::browser::Operation::Transcode,
                                )
                                .await
                            {
                                Ok(Some(path)) => Content::File {
                                    path,
                                    mime: mime.into(),
                                    name: filename.to_owned(),
                                    download: false,
                                },
                                _ => Content::TranscodingUnavailable,
                            },
                        );
                    }
                }
                if let Some(path) = state
                    .browser
                    .convert(&path, plain_rs::services::files::browser::Operation::Remux)
                    .await
                    .map_err(anyhow::Error::msg)?
                {
                    return Ok(Content::File {
                        path,
                        mime: mime.into(),
                        name: filename.to_owned(),
                        download: false,
                    });
                }
            }
        }
    }
    Ok(Content::File {
        path: PathBuf::from(&path),
        mime: mime.into(),
        name: filename.to_owned(),
        download: options.download,
    })
}

async fn read_slice(path: &str, offset: i64, length: usize) -> anyhow::Result<Content> {
    if length > 256 * 1024 {
        return Ok(Content::SliceTooLarge);
    }
    if offset < 0 {
        return Ok(Content::NotFound);
    }
    let Ok(mut file) = tokio::fs::File::open(path).await else {
        return Ok(Content::NotFound);
    };
    use tokio::io::{AsyncReadExt, AsyncSeekExt};
    file.seek(std::io::SeekFrom::Start(offset as u64)).await?;
    let mut bytes = vec![0; length];
    let mut n = 0;
    while n < length {
        let count = file.read(&mut bytes[n..]).await?;
        if count == 0 {
            break;
        }
        n += count;
    }
    bytes.truncate(n);
    Ok(Content::Bytes(bytes))
}

#[cfg(test)]
#[path = "../../../tests/unit/services/files/content.rs"]
mod tests;
