use crate::graphql::resolvers::files::types::AudioFileInfo;
use crate::graphql::resolvers::files::types::DriveType;
use crate::graphql::resolvers::files::types::File;
use crate::graphql::resolvers::files::types::FileInfo;
use crate::graphql::resolvers::files::types::ImageFileInfo;
use crate::graphql::resolvers::files::types::Location;
use crate::graphql::resolvers::files::types::MediaFileInfo;
use crate::graphql::resolvers::files::types::Mount;
use crate::graphql::resolvers::files::types::VideoFileInfo;
use crate::graphql::types::facts::flag;
use crate::graphql::types::facts::integer;
use crate::graphql::types::facts::list;
use crate::graphql::types::facts::rows;
use crate::graphql::types::facts::text;
use crate::http::auth::public_gate;
use async_graphql::{Context, Enum, Object};
use chrono::{DateTime, Utc};
use plain_rs::files::filesystem::SortBy;
use plain_rs::files::filesystem::browse::Request;
use plain_rs::prefs::settings::FileSortBy;
use plain_rs::services::files::browse as file_reads;
use plain_rs::types::Instant;
use plain_rs::types::Long;
use serde_json::{Value, json};
use std::{path::Path, sync::Arc};

const STORAGE: &str = "WRITE_EXTERNAL_STORAGE";

#[derive(Default)]
pub struct FilesQuery;

#[Object]
impl FilesQuery {
    async fn indexed_files(
        &self,
        ctx: &Context<'_>,
        query: String,
        offset: i32,
        limit: i32,
        sort_by: FileSortBy,
    ) -> async_graphql::Result<Vec<File>> {
        if !public_gate::require_granted(ctx, &[STORAGE]).await? {
            return Ok(Vec::new());
        }
        let items = ctx
            .data::<Arc<plain_rs::services::files::platform::FilePlatform>>()?
            .indexed_files(query, offset, limit, sort_by, false)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        Ok(rows(&items, file))
    }
    async fn indexed_file_count(
        &self,
        ctx: &Context<'_>,
        query: String,
    ) -> async_graphql::Result<Long> {
        if !public_gate::require_granted(ctx, &[STORAGE]).await? {
            return Ok(Long(0));
        }
        let count = ctx
            .data::<Arc<plain_rs::services::files::platform::FilePlatform>>()?
            .indexed_files(query, 0, 0, FileSortBy::NameAsc, true)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        Ok(Long(count.as_i64().ok_or("invalid indexed file count")?))
    }

    async fn mounts(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<Mount>> {
        let facts = ctx
            .data::<Arc<plain_rs::services::files::platform::FilePlatform>>()?
            .mounts(prefs(ctx).clone())
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        Ok(rows(&facts, mount))
    }

    /// Recently-opened files. Reading them is a storage read, so this one
    /// gates — plain-app's `recentFiles` calls `checkEnabledAsync`.
    async fn recent_files(&self, ctx: &Context<'_>) -> async_graphql::Result<Vec<File>> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        let items = ctx
            .data::<Arc<plain_rs::services::files::platform::FilePlatform>>()?
            .recent_files()
            .await?;
        Ok(items.iter().map(file).collect())
    }

    /// One page of a directory listing, filtered and sorted by the search
    /// DSL. `query` may carry a `parent` field to redirect the root, which
    /// is what the file manager's breadcrumb links send.
    async fn files(
        &self,
        ctx: &Context<'_>,
        root: String,
        query: String,
        offset: i32,
        limit: i32,
        sort_by: FileSortBy,
    ) -> async_graphql::Result<Vec<File>> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        let page = file_reads::execute(
            ctx.data::<Arc<plain_rs::services::files::platform::FilePlatform>>()?,
            Request {
                root,
                query,
                sort_by: sort_by.into(),
                offset: offset.max(0) as usize,
                limit: Some(limit.max(0) as usize),
                count_only: false,
            },
        )
        .await
        .map_err(async_graphql::Error::new)?;
        Ok(list(&page, "items", file))
    }

    /// Total matches for the same query, ignoring paging.
    async fn file_count(
        &self,
        ctx: &Context<'_>,
        root: String,
        query: String,
    ) -> async_graphql::Result<i32> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        let page = file_reads::execute(
            ctx.data::<Arc<plain_rs::services::files::platform::FilePlatform>>()?,
            Request {
                root,
                query,
                sort_by: SortBy::NameAsc,
                offset: 0,
                limit: None,
                count_only: true,
            },
        )
        .await
        .map_err(async_graphql::Error::new)?;
        Ok(page["count"].as_u64().unwrap_or_default() as i32)
    }

    async fn file_info(
        &self,
        ctx: &Context<'_>,
        path: String,
        file_name: Option<String>,
    ) -> async_graphql::Result<FileInfo> {
        public_gate::require(ctx, prefs(ctx), &[STORAGE])?;
        let record = file_reads::stat_record(
            ctx.data::<Arc<plain_rs::services::files::platform::FilePlatform>>()?,
            &path,
        )
        .await;
        let name = file_name.unwrap_or_else(|| {
            record
                .as_ref()
                .map(|record| record.name.clone())
                .unwrap_or_default()
        });
        let data = match &record {
            Some(record) if ctx.look_ahead().field("data").exists() => {
                media_info(ctx, &record.path, &name).await?
            }
            _ => None,
        };
        Ok(FileInfo {
            path,
            updated_at: record
                .as_ref()
                .map(|record| instant(record.updated_at))
                .unwrap_or_else(epoch),
            size: Long(record.map(|record| record.size).unwrap_or_default()),
            data,
        })
    }

    /// A total predicate, never an error: a blank path, `"."`, a missing
    /// file and a denied path all read as `false` so callers can use it in
    /// a filter without guarding every call.
    async fn path_exists(&self, ctx: &Context<'_>, path: String) -> async_graphql::Result<bool> {
        Ok(file_reads::stat_record(
            ctx.data::<Arc<plain_rs::services::files::platform::FilePlatform>>()?,
            &path,
        )
        .await
        .is_some())
    }

    /// Same total-predicate semantics as `pathExists`; `null` when the path
    /// cannot be resolved.
    async fn path_kind(
        &self,
        ctx: &Context<'_>,
        path: String,
    ) -> async_graphql::Result<Option<PathKind>> {
        Ok(file_reads::stat_record(
            ctx.data::<Arc<plain_rs::services::files::platform::FilePlatform>>()?,
            &path,
        )
        .await
        .map(|record| {
            if record.is_dir {
                PathKind::Dir
            } else {
                PathKind::File
            }
        }))
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
enum PathKind {
    File,
    Dir,
}

/// The probe is chosen by file *name*, not by sniffing: the client may pass
/// a display name that differs from the on-disk one, and plain-app keys off
/// the extension either way.
async fn media_info(
    ctx: &Context<'_>,
    path: &str,
    name: &str,
) -> async_graphql::Result<Option<MediaFileInfo>> {
    let extension = Path::new(name)
        .extension()
        .map(|extension| extension.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let kind = if is_image(&extension) {
        plain_rs::enums::DataType::Image
    } else if is_video(&extension) {
        plain_rs::enums::DataType::Video
    } else if is_audio(&extension) {
        plain_rs::enums::DataType::Audio
    } else {
        return Ok(None);
    };
    let selected = ctx.look_ahead().field("data");
    let selections = selected.selection_fields();
    let needs_details = selections
        .iter()
        .flat_map(|field| field.selection_set())
        .any(|field| !matches!(field.name(), "durationMs" | "__typename"));
    let facts = if !needs_details {
        if kind != plain_rs::enums::DataType::Image && selected.field("durationMs").exists() {
            match ctx
                .data::<Arc<plain_rs::services::files::duration_updates::Runtime>>()?
                .probe_milliseconds(path)
                .await
                .map_err(async_graphql::Error::new)?
            {
                Some(duration) => json!({"durationMs":duration}),
                None => ctx
                    .data::<Arc<plain_rs::services::files::info::Info>>()?
                    .read(path, kind)
                    .await
                    .map_err(async_graphql::Error::new)?,
            }
        } else {
            json!({})
        }
    } else {
        ctx.data::<Arc<plain_rs::services::files::info::Info>>()?
            .read(path, kind)
            .await
            .map_err(async_graphql::Error::new)?
    };
    if facts.is_null() {
        return Ok(None);
    }
    Ok(Some(match kind {
        plain_rs::enums::DataType::Image => MediaFileInfo::Image(ImageFileInfo {
            make: text(&facts, "make"),
            model: text(&facts, "model"),
            rotation: integer(&facts, "rotation") as i32,
            color_space: text(&facts, "colorSpace"),
            aperture_value: facts["apertureValue"].as_f64().unwrap_or(0.0),
            exposure_time: text(&facts, "exposureTime"),
            focal_length: text(&facts, "focalLength"),
            iso_speed: integer(&facts, "isoSpeed") as i32,
            taken_at: facts["takenAt"]
                .as_str()
                .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                .map(|value| plain_rs::types::Instant(value.with_timezone(&chrono::Utc))),
            flash: integer(&facts, "flash") as i32,
            f_number: facts["fNumber"].as_f64().unwrap_or(0.0),
            exposure_program: integer(&facts, "exposureProgram") as i32,
            metering_mode: integer(&facts, "meteringMode") as i32,
            white_balance: integer(&facts, "whiteBalance") as i32,
            creator: text(&facts, "creator"),
            resolution_x: integer(&facts, "resolutionX") as i32,
            resolution_y: integer(&facts, "resolutionY") as i32,
            description: text(&facts, "description"),

            width: integer(&facts, "width") as i32,
            height: integer(&facts, "height") as i32,
            location: location(&facts),
        }),
        plain_rs::enums::DataType::Video => MediaFileInfo::Video(VideoFileInfo {
            width: integer(&facts, "width") as i32,
            height: integer(&facts, "height") as i32,
            duration_ms: Long(integer(&facts, "durationMs")),
            rotation: integer(&facts, "rotation") as i32,
            display_width: integer(&facts, "displayWidth") as i32,
            display_height: integer(&facts, "displayHeight") as i32,
            bitrate: Long(integer(&facts, "bitrate")),
            frame_rate: facts["frameRate"].as_f64().unwrap_or(0.0),
            title: text(&facts, "title"),
            artist: text(&facts, "artist"),
            album: text(&facts, "album"),
            genre: text(&facts, "genre"),
            writer: text(&facts, "writer"),
            composer: text(&facts, "composer"),
            taken_at: facts["takenAt"]
                .as_str()
                .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                .map(|value| plain_rs::types::Instant(value.with_timezone(&chrono::Utc))),
            location: location(&facts),
        }),
        _ => MediaFileInfo::Audio(AudioFileInfo {
            duration_ms: Long(integer(&facts, "durationMs")),
            location: location(&facts),
        }),
    }))
}

fn location(value: &Value) -> Option<Location> {
    let raw = &value["location"];
    if let (Some(latitude), Some(longitude)) = (raw["latitude"].as_f64(), raw["longitude"].as_f64())
    {
        return Some(Location {
            latitude,
            longitude,
        });
    }
    let raw = value["rawLocation"].as_str()?;
    let (latitude, longitude) = plain_rs::utils::iso6709::coordinates(raw)?;
    Some(Location {
        latitude,
        longitude,
    })
}

/// Kept in step with plain-app's `isImageFast` / `isVideoFast` /
/// `isAudioFast`, which are extension checks over the same extension sets
/// the platform decoders register.
fn is_image(extension: &str) -> bool {
    matches!(
        extension,
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tif" | "tiff" | "heic" | "heif" | "svg"
    )
}

fn is_video(extension: &str) -> bool {
    matches!(
        extension,
        "mp4" | "m4v" | "mov" | "mkv" | "webm" | "avi" | "3gp" | "ts" | "flv" | "wmv"
    )
}

fn is_audio(extension: &str) -> bool {
    matches!(
        extension,
        "mp3" | "m4a" | "aac" | "wav" | "flac" | "ogg" | "opus" | "amr" | "wma" | "mid"
    )
}

fn file(item: &Value) -> File {
    File {
        permission: text(item, "permission"),
        file_id: item["mediaId"]
            .as_str()
            .filter(|value| !value.is_empty())
            .map(|value| async_graphql::ID::from(value)),
        name: text(item, "name"),
        path: text(item, "path"),
        created_at: item["createdAt"].as_i64().map(|millis| instant(millis)),
        updated_at: instant(item["updatedAt"].as_i64().unwrap_or_default()),
        size: Long(item["size"].as_i64().unwrap_or_default()),
        is_dir: flag(item, "isDir"),
        child_count: integer(item, "childCount") as i32,
    }
}

fn mount(item: &Value) -> Mount {
    Mount {
        id: async_graphql::ID(
            item.get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("path:{}", text(item, "path"))),
        ),
        name: text(item, "name"),
        path: text(item, "path"),
        mount_point: item
            .get("mountPoint")
            .and_then(Value::as_str)
            .unwrap_or_else(|| item.get("path").and_then(Value::as_str).unwrap_or_default())
            .to_owned(),
        fs_type: text(item, "fsType"),
        total_bytes: Long(integer(item, "totalBytes")),
        used_bytes: Long(
            item.get("usedBytes")
                .and_then(Value::as_i64)
                .unwrap_or_else(|| {
                    integer(item, "totalBytes")
                        .saturating_sub(integer(item, "freeBytes"))
                        .max(0)
                }),
        ),
        free_bytes: Long(integer(item, "freeBytes")),
        remote: flag(item, "remote"),
        alias: text(item, "alias"),
        drive_type: DriveType::parse(&text(item, "driveType")),
        disk_id: text(item, "diskId"),
    }
}

fn instant(millis: i64) -> Instant {
    Instant(DateTime::<Utc>::from_timestamp_millis(millis).unwrap_or_else(epoch_datetime))
}

fn epoch() -> Instant {
    Instant(epoch_datetime())
}

fn epoch_datetime() -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(0, 0).unwrap_or_default()
}

fn prefs<'a>(ctx: &'a Context<'_>) -> &'a Arc<plain_rs::prefs::Prefs> {
    ctx.data_unchecked::<Arc<plain_rs::prefs::Prefs>>()
}

#[cfg(test)]
#[path = "../../../../tests/unit/graphql/resolvers/files/public.rs"]
mod tests;
