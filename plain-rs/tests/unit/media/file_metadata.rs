use super::*;
use crate::db::MediaFileMetadata;
use crate::enums::DataType;
use crate::utils::dbtime::iso_from_unix_millis;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

fn read_file_metadata(path: &Path, data_type: DataType) -> anyhow::Result<MediaFileMetadata> {
    Ok(read_file_metadata_with_availability(path, data_type)?.metadata)
}

fn read_file_metadata_with_availability(
    path: &Path,
    data_type: DataType,
) -> anyhow::Result<crate::db::MediaFileMetadataRead> {
    super::read(path, data_type, |_| None)
}

#[test]
fn an_external_image_decoder_receives_the_rewound_file_only_when_header_dimensions_are_missing() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("image.other");
    std::fs::write(&path, b"external image codec fixture").unwrap();
    let calls = std::cell::Cell::new(0);
    let facts = super::read(&path, DataType::Image, |mut reader| {
        calls.set(calls.get() + 1);
        assert_eq!(reader.stream_position().unwrap(), 0);
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"external image codec fixture");
        Some((320, 240))
    })
    .unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!((facts.metadata.width, facts.metadata.height), (320, 240));
    assert!(facts.available.dimensions);
    let facts = super::read(&fixture("v-prog420.jpg"), DataType::Image, |_| {
        panic!("header dimensions already available")
    })
    .unwrap();
    assert!(facts.metadata.width > 0 && facts.metadata.height > 0);
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../plain-rs/testdata")).join(name)
}

fn dbtime_of(at: SystemTime) -> String {
    let millis = at
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    iso_from_unix_millis(millis)
}

fn listing(dir: &Path) -> Vec<(String, u64, SystemTime)> {
    let mut out: Vec<(String, u64, SystemTime)> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            let meta = entry.metadata().unwrap();
            (
                entry.file_name().to_string_lossy().into_owned(),
                meta.len(),
                meta.modified().unwrap(),
            )
        })
        .collect();
    out.sort();
    out
}

#[test]
fn accepted_kinds_are_exactly_the_media_index_wire_values() {
    let cases = [
        (DataType::Audio, "audio", "audio-tagged.mp3"),
        (DataType::Video, "video", "video-h265.mp4"),
        (DataType::Image, "image", "v-prog420.jpg"),
        (DataType::Doc, "doc", "html_to_markdown.md"),
    ];
    for (data_type, kind, name) in cases {
        assert_eq!(data_type.media_type_str(), Some(kind));
        let m = read_file_metadata(&fixture(name), data_type).unwrap();
        assert_eq!(m.data_type, data_type);
        assert_eq!(
            m.path,
            std::fs::canonicalize(fixture(name))
                .unwrap()
                .to_str()
                .unwrap()
        );
    }
}

#[test]
fn data_types_outside_the_media_index_are_errors_even_for_a_real_file() {
    let path = fixture("v-photo-prog.jpg");
    for data_type in [
        DataType::Default,
        DataType::Sms,
        DataType::Contact,
        DataType::Note,
        DataType::FeedEntry,
        DataType::Call,
        DataType::Package,
        DataType::File,
        DataType::AppFile,
    ] {
        assert!(
            read_file_metadata(&path, data_type).is_err(),
            "{data_type:?} has no media kind"
        );
    }
}

#[test]
fn audio_row_carries_probe_tags_and_real_stat_facts() {
    let path = fixture("audio-tagged.mp3");
    let canonical = std::fs::canonicalize(&path).unwrap();
    let meta = std::fs::metadata(&path).unwrap();

    let m = read_file_metadata(&path, DataType::Audio).unwrap();

    assert_eq!(m.data_type, DataType::Audio);
    assert_eq!(m.path, canonical.to_str().unwrap());
    assert_eq!(m.bucket_id, canonical.parent().unwrap().to_str().unwrap());
    assert_eq!(m.size_bytes, i64::try_from(meta.len()).unwrap());
    assert_eq!(m.artist, "Hydrate Artist");
    assert_eq!(m.title, "Hydrate Title");
    assert_eq!(m.cover_image_id, None);
    let probed = crate::media::probe::probe_media(m.path.as_str(), "audio");
    assert_eq!(m.duration_ms, probed.duration_ms);
    assert!(m.duration_ms > 0);
    assert_eq!((m.width, m.height, m.rotation), (0, 0, 0));
    assert_eq!(m.taken_at, None);
}

#[test]
fn timestamps_use_the_dbtime_row_format() {
    let path = fixture("v-prog-gray.jpg");
    let meta = std::fs::metadata(&path).unwrap();
    let modified = meta.modified().unwrap();

    let m = read_file_metadata(&path, DataType::Image).unwrap();

    assert_eq!(m.updated_at, dbtime_of(modified));
    assert!(m.updated_at.ends_with('Z'), "{}", m.updated_at);
    assert!(m.created_at.ends_with('Z'), "{}", m.created_at);
    let born = birth_unix_secs(&meta)
        .map(|secs| SystemTime::UNIX_EPOCH + Duration::from_secs(secs as u64));
    assert_eq!(m.created_at, dbtime_of(born.unwrap_or(modified)));
}

#[test]
fn created_at_falls_back_to_modified_when_the_os_records_no_birth_time() {
    let modified = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let born = SystemTime::UNIX_EPOCH + Duration::from_secs(1_600_000_000);

    assert_eq!(created_at_of(None, modified), "2023-11-14T22:13:20.000Z");
    assert_eq!(
        created_at_of(Some(born), modified),
        "2020-09-13T12:26:40.000Z"
    );
    assert_eq!(
        row_timestamp(SystemTime::UNIX_EPOCH),
        "1970-01-01T00:00:00.000Z"
    );
}

#[test]
fn timestamps_sort_chronologically_as_text() {
    let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let later = base + Duration::from_secs(1);

    assert!(row_timestamp(base) < row_timestamp(later));
}

#[test]
fn video_row_carries_container_duration_and_dimensions() {
    let m = read_file_metadata(&fixture("video-h264-baseline.mp4"), DataType::Video).unwrap();

    assert_eq!(m.duration_ms, 6_000);
    assert!(m.artist.is_empty());
    assert_eq!(m.title, "video-h264-baseline");
    assert_eq!((m.width, m.height, m.rotation), (320, 240, 0));
    assert_eq!(m.taken_at, None);
}

#[test]
fn video_catalog_dimensions_use_shared_container_readers_without_an_image_decoder() {
    for (name, width, height) in [
        ("video-h264-baseline.mp4", 320, 240),
        ("video-vp8.mkv", 16, 16),
        ("video-mjpeg.avi", 16, 16),
        ("video-wmv2.wmv", 16, 16),
    ] {
        let read = super::read(&fixture(name), DataType::Video, |_| {
            panic!("video metadata must not use an image decoder")
        })
        .unwrap();
        assert_eq!(
            (read.metadata.width, read.metadata.height),
            (width, height),
            "{name}"
        );
        assert!(read.available.dimensions, "{name}");
        assert!(read.available.rotation, "{name}");
    }
}

#[test]
fn unreadable_video_metadata_remains_unknown_without_invented_dimensions() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("broken.mp4");
    std::fs::write(&path, b"not a video").unwrap();
    let read = super::read(&path, DataType::Video, |_| {
        panic!("invalid video must not be treated as an image")
    })
    .unwrap();
    assert_eq!(
        (
            read.metadata.width,
            read.metadata.height,
            read.metadata.rotation
        ),
        (0, 0, 0)
    );
    assert!(!read.available.dimensions);
    assert!(!read.available.rotation);
    assert!(!read.available.duration);
}

#[test]
fn tagless_audio_falls_back_to_the_file_stem() {
    let m = read_file_metadata(&fixture("audio-untagged.mp3"), DataType::Audio).unwrap();

    assert_eq!(m.title, "audio-untagged");
    assert!(m.artist.is_empty());
    assert!(m.duration_ms > 0);
}

#[test]
fn doc_row_keeps_real_stat_facts_and_unknown_optional_metadata() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../schema/guest-api.graphqls");
    let m = read_file_metadata(&path, DataType::Doc).unwrap();

    assert_eq!(m.data_type, DataType::Doc);
    assert_eq!(m.title, "guest-api");
    assert!(m.artist.is_empty());
    assert_eq!(m.duration_ms, 0);
    assert_eq!((m.width, m.height, m.rotation), (0, 0, 0));
    assert_eq!(m.taken_at, None);
    assert!(m.size_bytes > 0);
    assert!(!m.created_at.is_empty() && !m.updated_at.is_empty());
}

#[test]
fn image_row_reads_pixel_dimensions_from_the_image_crate() {
    let big = read_file_metadata(&fixture("v-photo-prog.jpg"), DataType::Image).unwrap();
    assert_eq!((big.width, big.height), (1200, 800));
    assert_eq!(big.rotation, 0, "fixture carries no EXIF orientation");
    assert_eq!(big.taken_at, None);
    assert_eq!(big.title, "v-photo-prog");
    assert_eq!(big.duration_ms, 0);

    let small = read_file_metadata(&fixture("v-prog-gray.jpg"), DataType::Image).unwrap();
    assert_eq!((small.width, small.height), (160, 120));
}

#[test]
fn image_dimensions_follow_file_contents_instead_of_a_misleading_extension() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["photo.heic", "photo.avif", "photo.bmp", "photo.tiff"] {
        let path = dir.path().join(name);
        std::fs::copy(fixture("v-photo-prog.jpg"), &path).unwrap();

        let m = read_file_metadata(&path, DataType::Image).unwrap();

        assert_eq!((m.width, m.height, m.rotation), (1200, 800, 0), "{name}");
        assert_eq!(m.taken_at, None, "{name}");
        assert_eq!(m.title, "photo", "{name}");
        assert!(m.size_bytes > 0, "{name}");
        assert!(
            !m.created_at.is_empty() && !m.updated_at.is_empty(),
            "{name}"
        );
    }
}

#[test]
fn missing_path_and_directory_are_errors() {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        read_file_metadata(&dir.path().join("nope.jpg"), DataType::Image).is_err(),
        "missing file"
    );
    assert!(
        read_file_metadata(dir.path(), DataType::Image).is_err(),
        "directory is not a media file"
    );
    let file = dir.path().join("x.jpg");
    std::fs::write(&file, b"not an image").unwrap();
    assert!(
        read_file_metadata(&file, DataType::Image).is_ok(),
        "an undecodable image is still a real file with a row"
    );
}

#[cfg(unix)]
#[test]
fn paths_that_are_not_utf8_are_errors() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(OsStr::from_bytes(b"bad-\xFF-name.jpg"));
    let error = read_file_metadata(&path, DataType::Image).unwrap_err();
    assert!(error.to_string().contains("not valid UTF-8"));
}

#[cfg(unix)]
#[test]
fn an_unreadable_directory_is_an_error() {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt};

    let dir = tempfile::tempdir().unwrap();
    if dir.path().metadata().unwrap().uid() == 0 {
        return;
    }
    let locked = dir.path().join("locked");
    std::fs::create_dir(&locked).unwrap();
    let file = locked.join("a.jpg");
    std::fs::write(&file, b"x").unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();

    let result = read_file_metadata(&file, DataType::Image);

    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.is_err());
}

#[test]
fn reading_facts_writes_nothing_and_repeats_identically() {
    let dir = tempfile::tempdir().unwrap();
    let files = [
        ("audio-tagged.mp3", DataType::Audio),
        ("video-h265.mp4", DataType::Video),
        ("v-prog-gray.jpg", DataType::Image),
        ("html_to_markdown.md", DataType::Doc),
    ];
    for (name, _) in files {
        std::fs::copy(fixture(name), dir.path().join(name)).unwrap();
    }
    let before = listing(dir.path());

    let read_all = || {
        files
            .iter()
            .map(|(name, data_type)| {
                read_file_metadata(&dir.path().join(*name), *data_type).unwrap()
            })
            .collect::<Vec<MediaFileMetadata>>()
    };
    let first = read_all();
    let second = read_all();

    assert_eq!(format!("{first:?}"), format!("{second:?}"));
    assert_eq!(
        before,
        listing(dir.path()),
        "no file was created or touched"
    );
}

#[test]
fn probe_availability_keeps_unknown_distinct_from_valid_untagged_audio() {
    let directory = tempfile::tempdir().unwrap();
    let damaged = directory.path().join("damaged.mp3");
    std::fs::write(&damaged, b"unrecognized audio bytes").unwrap();
    let unknown = read_file_metadata_with_availability(&damaged, DataType::Audio).unwrap();
    assert!(!unknown.available.tags);
    assert!(!unknown.available.duration);
    assert!(!unknown.available.cover_image_id);
    let untagged =
        read_file_metadata_with_availability(&fixture("audio-untagged.mp3"), DataType::Audio)
            .unwrap();
    assert!(untagged.available.tags);
    assert!(untagged.available.duration);
    assert!(untagged.metadata.artist.is_empty());
}

#[test]
fn unsupported_image_metadata_stays_visible_without_reporting_unknown_fields_as_available() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("unknown.heic");
    std::fs::write(&path, b"unsupported image header").unwrap();
    let facts = read_file_metadata_with_availability(&path, DataType::Image).unwrap();
    assert_eq!(facts.metadata.data_type, DataType::Image);
    assert!(facts.available.tags);
    assert!(!facts.available.dimensions);
    assert!(!facts.available.rotation);
    assert!(!facts.available.taken_at);
}

fn distant_image_metadata(orientation: u16) -> Vec<u8> {
    let first = 1024 * 1024;
    let payload = 2 * 1024 * 1024;
    let mut bytes = vec![0; 3 * 1024 * 1024];
    bytes[..4].copy_from_slice(b"II\x2a\0");
    bytes[4..8].copy_from_slice(&(first as u32).to_le_bytes());
    bytes[first..first + 2].copy_from_slice(&5u16.to_le_bytes());
    for (index, tag, kind, count, value) in [
        (0, 0x0100u16, 4u16, 1u32, 1920u32),
        (1, 0x0101, 4, 1, 1080),
        (2, 0x0112, 3, 1, u32::from(orientation)),
        (3, 0x010f, 2, 7, payload as u32),
        (4, 0x8769, 4, 1, (first + 256) as u32),
    ] {
        let start = first + 2 + index * 12;
        bytes[start..start + 2].copy_from_slice(&tag.to_le_bytes());
        bytes[start + 2..start + 4].copy_from_slice(&kind.to_le_bytes());
        bytes[start + 4..start + 8].copy_from_slice(&count.to_le_bytes());
        bytes[start + 8..start + 12].copy_from_slice(&value.to_le_bytes());
    }
    let exif = first + 256;
    bytes[exif..exif + 2].copy_from_slice(&2u16.to_le_bytes());
    for (index, tag, count, value) in [
        (0, 0x9003u16, 20u32, (payload + 64) as u32),
        (1, 0x9011, 7, (payload + 96) as u32),
    ] {
        let start = exif + 2 + index * 12;
        bytes[start..start + 2].copy_from_slice(&tag.to_le_bytes());
        bytes[start + 2..start + 4].copy_from_slice(&2u16.to_le_bytes());
        bytes[start + 4..start + 8].copy_from_slice(&count.to_le_bytes());
        bytes[start + 8..start + 12].copy_from_slice(&value.to_le_bytes());
    }
    bytes[payload..payload + 7].copy_from_slice(b"Camera\0");
    bytes[payload + 64..payload + 84].copy_from_slice(b"2026:10:16 12:34:56\0");
    bytes[payload + 96..payload + 103].copy_from_slice(b"+08:00\0");
    bytes
}

#[test]
fn indexed_images_use_shared_seekable_exif_dates_and_rotation_without_swapping_raw_dimensions() {
    let directory = tempfile::tempdir().unwrap();
    for (orientation, rotation, available) in
        [(0, 0, false), (1, 0, true), (5, 0, true), (6, 90, true)]
    {
        let path = directory.path().join(format!("photo-{orientation}.tiff"));
        std::fs::write(&path, distant_image_metadata(orientation)).unwrap();
        let facts = read_file_metadata_with_availability(&path, DataType::Image).unwrap();
        assert_eq!(
            (
                facts.metadata.width,
                facts.metadata.height,
                facts.metadata.rotation
            ),
            (1920, 1080, rotation)
        );
        assert_eq!(facts.available.rotation, available);
        assert!(facts.available.dimensions);
        assert!(facts.available.taken_at);
        assert_eq!(
            facts.metadata.taken_at.as_deref(),
            Some("2026-10-16T04:34:56.000Z")
        );
    }
}

struct CountedImage {
    source: std::io::Cursor<Vec<u8>>,
    bytes_read: usize,
}
impl Read for CountedImage {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let count = self.source.read(bytes)?;
        self.bytes_read += count;
        Ok(count)
    }
}
impl Seek for CountedImage {
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
        self.source.seek(from)
    }
}

#[test]
fn indexed_image_metadata_seeks_to_distant_fields_without_reading_the_pixel_payload() {
    let mut reader = CountedImage {
        source: std::io::Cursor::new(distant_image_metadata(6)),
        bytes_read: 0,
    };
    let (width, height, rotation, taken_at) = image_reader(&mut reader, |_| None);
    assert_eq!((width, height, rotation), (1920, 1080, Some(90)));
    assert_eq!(taken_at.as_deref(), Some("2026-10-16T04:34:56.000Z"));
    assert!(
        reader.bytes_read < 140 * 1024,
        "{} bytes read",
        reader.bytes_read
    );
}

#[test]
fn video_row_keeps_fractional_milliseconds_from_the_single_probe() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("sample.mp4");
    let mut time = vec![0; 12];
    time.extend(1000u32.to_be_bytes());
    time.extend(3211u32.to_be_bytes());
    let mvhd = [28u32.to_be_bytes().to_vec(), b"mvhd".to_vec(), time].concat();
    std::fs::write(
        &path,
        [36u32.to_be_bytes().to_vec(), b"moov".to_vec(), mvhd].concat(),
    )
    .unwrap();
    let probe = crate::media::probe::probe_media(path.to_str().unwrap(), "video");
    assert_eq!(probe.duration_ms, 3211);
    assert_eq!(probe.duration_secs, 3);
    assert_eq!(
        read_file_metadata(&path, DataType::Video)
            .unwrap()
            .duration_ms,
        3211
    );
}

#[test]
fn audio_catalog_metadata_keeps_album_tags_from_the_shared_parser() {
    let facts =
        read_file_metadata_with_availability(&fixture("audio-album-tagged.mp3"), DataType::Audio)
            .unwrap();
    assert!(facts.available.tags);
    assert_eq!(facts.metadata.album, "Shared Album");
    assert_eq!(facts.metadata.album_artist, "Album Artist");
    assert_eq!(facts.metadata.artist, "Track Artist");
}
