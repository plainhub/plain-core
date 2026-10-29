use crate::db::{MediaFileMetadata, MediaFileMetadataAvailability, MediaFileMetadataRead};
use crate::enums::DataType;
use anyhow::{Context, Result, bail};
use std::fs::{File, Metadata};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::time::{Duration, SystemTime};

pub fn read(
    path: &Path,
    data_type: DataType,
    decode_dimensions: impl FnOnce(File) -> Option<(u32, u32)>,
) -> Result<MediaFileMetadataRead> {
    path.to_str().context("media path is not valid UTF-8")?;
    let kind = data_type
        .media_type_str()
        .with_context(|| format!("{data_type:?} is not a media data type"))?;
    let canonical = path
        .canonicalize()
        .with_context(|| format!("cannot resolve media path {}", path.display()))?;
    let meta = std::fs::metadata(&canonical)
        .with_context(|| format!("cannot stat {}", canonical.display()))?;
    if !meta.is_file() {
        bail!("{} is not a regular file", canonical.display());
    }
    let path_str = canonical
        .to_str()
        .with_context(|| format!("{} is not valid UTF-8", canonical.display()))?
        .to_string();
    let bucket_id = canonical
        .parent()
        .and_then(|p| p.to_str())
        .with_context(|| format!("{path_str} has no parent directory"))?
        .to_string();
    let size_bytes = i64::try_from(meta.len())
        .with_context(|| format!("{path_str} is larger than the row can store"))?;
    let modified = meta
        .modified()
        .with_context(|| format!("{path_str} has no readable modified time"))?;

    let probed = super::probe::probe_media(&path_str, kind);
    let duration_ms = probed.duration_ms;
    let (width, height, rotation, taken_at) = visual_facts(&canonical, kind, decode_dimensions);
    let available = MediaFileMetadataAvailability {
        tags: kind != "audio" || probed.parsed,
        duration: !matches!(kind, "audio" | "video") || probed.duration_available,
        dimensions: width > 0 && height > 0,
        rotation: rotation.is_some(),
        taken_at: taken_at.is_some(),
        cover_image_id: false,
    };
    Ok(MediaFileMetadataRead {
        file_identity: crate::utils::file_identity::within_volume_key(&canonical)?,
        available,
        metadata: MediaFileMetadata {
            data_type,
            title: if probed.title.is_empty() {
                file_stem(&path_str)
            } else {
                probed.title
            },
            artist: probed.artist,
            album: probed.album,
            album_artist: probed.album_artist,
            path: path_str,
            bucket_id,
            size_bytes,
            duration_ms,
            cover_image_id: None,
            width,
            height,
            rotation: rotation.unwrap_or_default(),
            taken_at,
            created_at: created_at_of(birth_time(&meta), modified),
            updated_at: row_timestamp(modified),
        },
    })
}

fn file_stem(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
        .to_string()
}

fn row_timestamp(at: SystemTime) -> String {
    let millis = at
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|since| i64::try_from(since.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or_default();
    crate::utils::dbtime::iso_from_unix_millis(millis)
}

fn created_at_of(birth: Option<SystemTime>, modified: SystemTime) -> String {
    birth.map_or_else(|| row_timestamp(modified), row_timestamp)
}

fn birth_unix_secs(meta: &Metadata) -> Option<i64> {
    meta.created()
        .ok()
        .and_then(|created| created.duration_since(SystemTime::UNIX_EPOCH).ok())
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .filter(|seconds| *seconds > 0)
}

fn birth_time(meta: &Metadata) -> Option<SystemTime> {
    birth_unix_secs(meta).map(|secs| SystemTime::UNIX_EPOCH + Duration::from_secs(secs as u64))
}

fn visual_facts(
    path: &Path,
    kind: &str,
    decode_dimensions: impl FnOnce(File) -> Option<(u32, u32)>,
) -> (i32, i32, Option<i32>, Option<String>) {
    if kind == "video" {
        return File::open(path)
            .ok()
            .and_then(|mut reader| super::video_info::read(&mut reader))
            .and_then(|video| {
                Some((
                    i32::try_from(video.width).ok()?,
                    i32::try_from(video.height).ok()?,
                    Some(video.rotation),
                    None,
                ))
            })
            .unwrap_or_default();
    }
    image_facts(path, kind, decode_dimensions)
}

fn image_facts(
    path: &Path,
    kind: &str,
    decode_dimensions: impl FnOnce(File) -> Option<(u32, u32)>,
) -> (i32, i32, Option<i32>, Option<String>) {
    if kind != "image" {
        return (0, 0, None, None);
    }
    File::open(path)
        .map(|reader| image_reader(reader, decode_dimensions))
        .unwrap_or((0, 0, None, None))
}

fn image_reader<R: Read + Seek>(
    mut reader: R,
    decode_dimensions: impl FnOnce(R) -> Option<(u32, u32)>,
) -> (i32, i32, Option<i32>, Option<String>) {
    let mut head = Vec::new();
    if reader
        .by_ref()
        .take(128 * 1024)
        .read_to_end(&mut head)
        .is_err()
    {
        return (0, 0, None, None);
    }
    let exif = crate::utils::image_exif::read(&mut reader).ok().flatten();
    let metadata = crate::utils::image_metadata::read(exif.as_deref().unwrap_or(&head));
    let dimensions = crate::utils::image_dimensions::dimensions(&head)
        .or_else(|| {
            exif.as_deref()
                .and_then(crate::utils::image_dimensions::dimensions)
        })
        .filter(|(width, height)| *width > 0 && *height > 0)
        .or_else(|| {
            reader.seek(SeekFrom::Start(0)).ok()?;
            let (width, height) = decode_dimensions(reader)?;
            Some((i32::try_from(width).ok()?, i32::try_from(height).ok()?))
        });
    let (width, height) = dimensions.unwrap_or((0, 0));
    (
        width,
        height,
        metadata.orientation.map(|_| metadata.rotation),
        metadata.taken_at(),
    )
}

#[cfg(test)]
#[path = "../../tests/unit/media/file_metadata.rs"]
mod tests;
