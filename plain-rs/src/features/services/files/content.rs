use crate::features::services::files::links;
use crate::features::services::files::previews;
use crate::runtime::Runtime as CoreRuntime;
use plain_libs::serde_json::json;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
pub enum Size {
    #[serde(rename = "sm")]
    Small,
    #[serde(rename = "lg")]
    Large,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Describe,
    Preview,
    Download,
    Probe,
    Transcode,
}

impl Action {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Describe => "describe",
            Self::Preview => "preview",
            Self::Download => "download",
            Self::Probe => "probe",
            Self::Transcode => "transcode",
        }
    }
}

pub struct Options {
    pub access_order: u64,
    pub action: Action,
    pub size: Option<Size>,
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
    Codec(String),
    Markdown {
        text: String,
        name: String,
    },
    Thumbnail {
        bytes: Result<Option<bytes::Bytes>, crate::features::services::images::thumbnails::Error>,
        name: String,
    },
    NotFound,
    Directory,
    TranscodingUnavailable,
}

pub fn thumbnail_mime(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG") {
        "image/png"
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "image/gif"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else {
        "image/jpeg"
    }
}

pub async fn read(
    state: &CoreRuntime,
    file: links::File,
    options: Options,
) -> plain_libs::anyhow::Result<Content> {
    let download = options.action == Action::Download;
    let preview = options.action == Action::Preview;
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
            .map_err(plain_libs::anyhow::Error::msg)?;
        let filename = if name.is_empty() {
            path.trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or_default()
        } else {
            &name
        };
        if preview && options.size.is_none() && super::docs_preview::supports_name(filename) {
            return super::docs_preview::read(state, &cached.to_string_lossy())
                .await
                .map(|text| Content::Markdown {
                    text,
                    name: filename.to_owned(),
                });
        }
        return Ok(Content::File {
            mime: plain_libs::utils::mime::mime_from_ext(&cached.to_string_lossy()).to_owned(),
            path: cached,
            name: filename.to_owned(),
            download,
        });
    }
    if path.starts_with("content://") || path.starts_with("ph://") {
        if options.action == Action::Probe {
            return read_codec(state, &path).await;
        }
        if options.action == Action::Transcode {
            return Ok(Content::TranscodingUnavailable);
        }

        if preview && options.size.is_some() && path.starts_with("content://") {
            let mime = super::resources::read(&state.host, "mime", &path, json!({}))
                .await
                .map_err(plain_libs::anyhow::Error::msg)?;
            if mime.as_str().is_some_and(|value| value.starts_with("audio/"))
                || plain_libs::utils::mime::mime_from_ext(filename).starts_with("audio/")
            {
                let cover = crate::features::services::audio::covers::read(
                    &state.resources, state.cache.root(), &path,
                )
                .await
                .map_err(plain_libs::anyhow::Error::msg)?;
                return Ok(match cover {
                    Some(bytes) => Content::Thumbnail {
                        bytes: Ok(Some(bytes.into())),
                        name: filename.to_owned(),
                    },
                    None => Content::NotFound,
                });
            }
        }

        if preview && options.size.is_none() {
            let document = if super::docs_preview::supports_name(filename) {
                true
            } else if path.starts_with("content://") {
                let mime = super::resources::read(&state.host, "mime", &path, json!({}))
                    .await
                    .map_err(plain_libs::anyhow::Error::msg)?;
                mime.as_str()
                    .is_some_and(super::docs_preview::supports_mime)
            } else {
                false
            };
            if document {
                return super::docs_preview::read(state, &path).await.map(|text| {
                    Content::Markdown {
                        text,
                        name: filename.to_owned(),
                    }
                });
            }
        }
        if !download
            && path.starts_with("content://")
            && let Some(file) =
                previews::convert(state, previews::Conversion::Video3gp, &path).await?
        {
            return Ok(Content::Temporary {
                file,
                mime: "video/mp4",
                name: None,
            });
        }
        return Ok(Content::Resource {
            name: download.then(|| filename.to_owned()),
            path,
            download,
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
    if options.starts_view && options.size != Some(Size::Small) {
        state.file_platform.recent_files.add(&path);
    }
    if preview && options.size.is_none() && super::docs_preview::supports_name(filename) {
        return super::docs_preview::read(state, &path)
            .await
            .map(|text| Content::Markdown {
                text,
                name: filename.to_owned(),
            });
    }
    let mime = plain_libs::utils::mime::mime_from_ext(&path);
    if options.action == Action::Probe {
        return read_codec(state, &path).await;
    }
    if options.action == Action::Transcode && !mime.starts_with("video/") {
        return Ok(Content::TranscodingUnavailable);
    }
    if !download && mime != "image/x-icon" {
        let animated = if mime.starts_with("image/") {
            crate::features::services::files::resources::read(
                &state.host,
                "animated",
                &path,
                json!({"fileName":plain_libs::utils::query::url_encode(filename)}),
            )
            .await
            .map_err(plain_libs::anyhow::Error::msg)?
            .as_bool()
                == Some(true)
        } else {
            false
        };
        if !animated {
            if let Some(size) = options.size {
                let pixels = match size {
                    Size::Small => crate::features::services::images::thumbnails::THUMBNAIL_SIZE,
                    Size::Large => 1024,
                };
                let bytes = state
                    .thumbnails
                    .get(
                        state,
                        crate::features::services::images::thumbnails::Request {
                            access_order: Some(options.access_order),
                            path: path.clone(),
                            width: pixels,
                            height: pixels,
                            center_crop: !mime.starts_with("image/") && size == Size::Small,
                            file_id: media_id,
                            file_name: plain_libs::utils::query::url_encode(filename),
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
                if options.action == Action::Transcode {
                    let codec = crate::features::services::files::resources::read(
                        &state.host,
                        "probe",
                        &path,
                        json!({}),
                    )
                    .await
                    .map_err(plain_libs::anyhow::Error::msg)?;
                    if matches!(codec.as_str(), Some("hvc1" | "hev1")) {
                        return Ok(
                            match state
                                .browser
                                .convert(
                                    &path,
                                    crate::features::services::files::browser::Operation::Transcode,
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
                    .convert(
                        &path,
                        crate::features::services::files::browser::Operation::Remux,
                    )
                    .await
                    .map_err(plain_libs::anyhow::Error::msg)?
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
        download,
    })
}

async fn read_codec(state: &CoreRuntime, path: &str) -> plain_libs::anyhow::Result<Content> {
    let codec =
        crate::features::services::files::resources::read(&state.host, "probe", path, json!({}))
            .await
            .map_err(plain_libs::anyhow::Error::msg)?;
    Ok(Content::Codec(
        codec
            .as_str()
            .unwrap_or_default()
            .chars()
            .filter(|ch| ch.is_alphanumeric())
            .collect(),
    ))
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/services/files/content.rs"]
mod tests;
