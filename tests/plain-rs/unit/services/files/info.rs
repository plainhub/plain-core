use super::*;
use std::sync::Arc;

#[test]
fn video_projection_preserves_raw_dimensions_and_normalizes_rotation_and_time() {
    for (rotation, width, height) in [
        (90i64, 1080, 1920),
        (-90, 1080, 1920),
        (360, 1920, 1080),
        (180, 1920, 1080),
    ] {
        let mut facts = json!({"width":1920,"height":1080,"rotation":rotation,"durationMs":5_000_000_001i64,"bitrate":8_000_000,"frameRate":29.97,"rawTakenAt":"20261016T123456.123+0800","title":"title","artist":"artist"});
        normalize_video(&mut facts);
        assert_eq!(facts["width"], 1920);
        assert_eq!(facts["height"], 1080);
        assert_eq!(facts["displayWidth"], width);
        assert_eq!(facts["displayHeight"], height);
        assert_eq!(facts["rotation"], rotation.rem_euclid(360));
        assert_eq!(facts["durationMs"], 5_000_000_001i64);
        assert_eq!(facts["takenAt"], "2026-10-16T04:34:56.123Z");
        assert_eq!(facts["frameRate"], 29.97);
        assert_eq!(facts["title"], "title");
    }
}

#[test]
fn sdk_capture_dates_keep_offsets_and_fractional_seconds() {
    for (raw, expected) in [
        ("20261016T123456.789Z", "2026-10-16T12:34:56.789Z"),
        ("20261016T123456Z", "2026-10-16T12:34:56.000Z"),
        ("2026-10-16T12:34:56-04:00", "2026-10-16T16:34:56.000Z"),
    ] {
        assert_eq!(taken_at(raw).as_deref(), Some(expected));
    }
    assert!(taken_at("20269916T123456.123Z").is_none());
}

#[test]
fn image_metadata_prefix_never_reads_a_whole_large_file() {
    struct Reader {
        read: usize,
    }
    impl std::io::Read for Reader {
        fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
            self.read += bytes.len();
            bytes.fill(0);
            Ok(bytes.len())
        }
    }
    let mut reader = Reader { read: 0 };
    assert_eq!(image_prefix(&mut reader).unwrap().len(), 128 * 1024);
    assert_eq!(reader.read, 128 * 1024);
}

#[tokio::test]
async fn unsupported_video_returns_null_and_local_images_use_rust_without_host_metadata() {
    let host = Arc::new(Host::default());
    let (_generation, mut requests) = host.connect();
    let info = Info::sdk(host);
    let directory = tempfile::tempdir().unwrap();
    let video = directory.path().join("unsupported.mp4");
    std::fs::write(&video, b"unsupported container").unwrap();
    assert!(
        info.read(video.to_str().unwrap(), DataType::Video)
            .await
            .unwrap()
            .is_null()
    );
    let image = directory.path().join("image.png");
    image::RgbImage::new(3, 2).save(&image).unwrap();
    let facts = info
        .read(image.to_str().unwrap(), DataType::Image)
        .await
        .unwrap();
    assert_eq!(facts["width"], 3);
    assert_eq!(facts["height"], 2);
    for kind in [DataType::Audio, DataType::Video, DataType::Image] {
        assert!(
            info.read(directory.path().join("missing").to_str().unwrap(), kind)
                .await
                .is_err()
        );
    }
    assert!(requests.try_recv().is_err());
}

#[tokio::test]
async fn sdk_images_with_supported_headers_skip_host_metadata_calls() {
    let host = Arc::new(Host::default());
    let (_generation, mut requests) = host.connect();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("sample.png");
    image::RgbImage::new(3, 2).save(&path).unwrap();
    let facts = Info::sdk(host)
        .read(path.to_str().unwrap(), DataType::Image)
        .await
        .unwrap();
    assert_eq!(facts["width"], 3);
    assert_eq!(facts["height"], 2);
    assert!(requests.try_recv().is_err());
}
#[tokio::test]
async fn distant_tiff_fields_are_served_by_rust_without_sdk_metadata_fallback() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("distant.tiff");
    let bytes = distant_tiff_bytes();
    std::fs::write(&path, bytes).unwrap();
    let host = Arc::new(Host::default());
    let (_generation, mut requests) = host.connect();
    let facts = Info::sdk(host)
        .read(path.to_str().unwrap(), DataType::Image)
        .await
        .unwrap();
    assert_eq!(facts["make"], "Camera");
    assert_eq!(facts["rotation"], 90);
    assert_eq!(facts["width"], 1080);
    assert_eq!(facts["height"], 1920);
    assert!(requests.try_recv().is_err());
}

fn distant_tiff_bytes() -> Vec<u8> {
    let mut bytes = vec![0; 3 * 1024 * 1024];
    bytes[..8].copy_from_slice(&[b'I', b'I', 42, 0, 0, 0, 16, 0]);
    let first = 1024 * 1024;
    bytes[first..first + 2].copy_from_slice(&4_u16.to_le_bytes());
    for (index, tag, kind, count, value) in [
        (0, 0x0100_u16, 4_u16, 1_u32, 1920_u32),
        (1, 0x0101, 4, 1, 1080),
        (2, 0x010f, 2, 7, 2 * 1024 * 1024),
        (3, 0x0112, 3, 1, 6),
    ] {
        let offset = first + 2 + index * 12;
        bytes[offset..offset + 2].copy_from_slice(&tag.to_le_bytes());
        bytes[offset + 2..offset + 4].copy_from_slice(&kind.to_le_bytes());
        bytes[offset + 4..offset + 8].copy_from_slice(&count.to_le_bytes());
        bytes[offset + 8..offset + 12].copy_from_slice(&value.to_le_bytes());
    }
    bytes[2 * 1024 * 1024..2 * 1024 * 1024 + 7].copy_from_slice(b"Camera\0");
    bytes
}

#[tokio::test]
async fn opaque_image_descriptors_use_logical_ranges_and_the_same_rust_exif_path() {
    let directory = tempfile::tempdir().unwrap();
    let bytes = distant_tiff_bytes();
    let source = directory.path().join("resource");
    let mut wrapped = b"private-prefix".to_vec();
    let offset = wrapped.len();
    wrapped.extend_from_slice(&bytes);
    wrapped.extend_from_slice(b"private-suffix");
    std::fs::write(&source, wrapped).unwrap();
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let resources = Arc::new(plain_rs::host::resources::NativeResources::new(
        host.clone(),
    ));
    let mut info = Info::new(
        host.clone(),
        resources.clone(),
        directory.path().join("cache"),
    );
    Arc::get_mut(&mut info).unwrap().sdk = true;
    let reading =
        tokio::spawn(async move { info.read("content://fixture/image", DataType::Image).await });
    let request = requests.recv().await.unwrap();
    assert_eq!(request["method"], "fileResourceStream");
    let stream = resources
        .attach_native(request["params"]["id"].as_str().unwrap(), generation)
        .unwrap();
    host.reply(generation, json!({"id":request["id"],"result":null}))
        .unwrap();
    let metadata: Value =
        serde_json::from_str(&stream.next_request().await.unwrap().unwrap()).unwrap();
    assert_eq!(
        metadata,
        json!({"path":"content://fixture/image","file":true})
    );
    stream
        .file(
            plain_rs::host::resource_stream::NativeResourceFile::new(
                std::fs::File::open(source).unwrap(),
                offset as i64,
                bytes.len() as i64,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let facts = reading.await.unwrap().unwrap();
    assert_eq!(facts["make"], "Camera");
    assert_eq!(facts["rotation"], 90);
    assert_eq!(facts["width"], 1080);
    assert_eq!(facts["height"], 1920);
    assert!(requests.try_recv().is_err());
    assert!(
        !directory.path().join("cache").exists(),
        "seekable resources must not be copied to a staging file"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn nonseekable_opaque_image_streams_are_prepared_in_rust_and_cleaned_after_reading() {
    use std::{
        io::Write,
        os::{fd::OwnedFd, unix::net::UnixStream},
    };
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("image.png");
    image::RgbImage::new(3, 2).save(&source).unwrap();
    let bytes = std::fs::read(source).unwrap();
    let length = bytes.len();
    let (read, mut write) = UnixStream::pair().unwrap();
    let writer = std::thread::spawn(move || {
        write.write_all(b"before").unwrap();
        write.write_all(&bytes).unwrap();
        write.write_all(b"after").unwrap();
    });
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let resources = Arc::new(plain_rs::host::resources::NativeResources::new(
        host.clone(),
    ));
    let info = Info::new(
        host.clone(),
        resources.clone(),
        directory.path().join("cache"),
    );
    let reading =
        tokio::spawn(async move { info.read("content://fixture/pipe", DataType::Image).await });
    let request = requests.recv().await.unwrap();
    let stream = resources
        .attach_native(request["params"]["id"].as_str().unwrap(), generation)
        .unwrap();
    host.reply(generation, json!({"id":request["id"],"result":null}))
        .unwrap();
    stream.next_request().await.unwrap().unwrap();
    stream
        .file(
            plain_rs::host::resource_stream::NativeResourceFile::new(
                std::fs::File::from(OwnedFd::from(read)),
                6,
                length as i64,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let facts = reading.await.unwrap().unwrap();
    assert_eq!(facts["width"], 3);
    assert_eq!(facts["height"], 2);
    assert!(requests.try_recv().is_err());
    assert_eq!(
        std::fs::read_dir(directory.path().join("cache"))
            .unwrap()
            .count(),
        0
    );
    writer.join().unwrap();
}

#[test]
fn sdk_decode_bounds_only_fill_dimensions_and_preserve_rust_metadata() {
    let mut facts = json!({"width":0,"height":0,"rotation":90,"location":{"latitude":1.5,"longitude":-2.5},"make":"camera","takenAt":"2026-10-16T04:34:56.000Z"});
    apply_image_bounds(&mut facts, json!({"width":1920,"height":1080})).unwrap();
    assert_eq!(facts["width"], 1080);
    assert_eq!(facts["height"], 1920);
    assert_eq!(facts["location"], json!({"latitude":1.5,"longitude":-2.5}));
    assert_eq!(facts["make"], "camera");
    assert_eq!(facts["takenAt"], "2026-10-16T04:34:56.000Z");
    let saved = facts.clone();
    for sample in [json!({"width":0,"height":1080}), Value::Null] {
        apply_image_bounds(&mut facts, sample).unwrap();
        assert_eq!(facts, saved);
    }
}

#[test]
fn malformed_decode_bounds_cannot_overwrite_rust_metadata() {
    for sample in [
        json!({}),
        json!({"width":1,"height":-1}),
        json!({"width":1.5,"height":2}),
        json!({"width":2147483648i64,"height":2}),
        json!([1, 2]),
        json!({"width":1,"height":2,"location":{"latitude":9,"longitude":9}}),
    ] {
        let mut facts = json!({"width":32,"height":16,"rotation":0});
        let original = facts.clone();
        assert!(apply_image_bounds(&mut facts, sample).is_err());
        assert_eq!(facts, original);
    }
    let mut absent = Value::Null;
    apply_image_bounds(&mut absent, Value::Null).unwrap();
    assert_eq!(absent, json!({"width":0,"height":0}));
}

#[tokio::test]
async fn sdk_audio_details_parse_container_duration_and_raw_location_without_metadata_host_calls() {
    fn atom(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut bytes = (8 + data.len() as u32).to_be_bytes().to_vec();
        bytes.extend(kind);
        bytes.extend(data);
        bytes
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("recording.m4a");
    let raw = "+12.345678-123.456789/";
    let mut location = (raw.len() as u16).to_be_bytes().to_vec();
    location.extend([0, 0]);
    location.extend(raw.as_bytes());
    let mut time = vec![0; 12];
    time.extend(1000u32.to_be_bytes());
    time.extend(3211u32.to_be_bytes());
    std::fs::write(
        &path,
        atom(
            b"moov",
            &[
                atom(b"mvhd", &time),
                atom(b"udta", &atom(b"\xa9xyz", &location)),
            ]
            .concat(),
        ),
    )
    .unwrap();
    let host = Arc::new(Host::default());
    let (_generation, mut requests) = host.connect();
    let info = Info::sdk(host);
    let facts = info
        .read(path.to_str().unwrap(), DataType::Audio)
        .await
        .unwrap();
    assert_eq!(facts["durationMs"], 3211);
    assert_eq!(facts["rawLocation"], raw);
    assert!(requests.try_recv().is_err());
}

#[tokio::test]
async fn sdk_aac_and_amr_details_use_the_shared_rust_duration_reader() {
    let directory = tempfile::tempdir().unwrap();
    let mut aac = vec![0; 16];
    aac[..7].copy_from_slice(&[0xff, 0xf1, 0x4c, 0x80, 0x02, 0x1f, 0xfc]);
    let mut amr = b"#!AMR-WB\n".to_vec();
    for _ in 0..100 {
        amr.extend([0x4c, 0, 0, 0, 0, 0]);
    }
    let host = Arc::new(Host::default());
    let (_generation, mut requests) = host.connect();
    let info = Info::sdk(host);
    for (name, bytes, expected) in [
        ("recording.aac", aac.repeat(100), 2133),
        ("recording.amr", amr, 2000),
    ] {
        let path = directory.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        let facts = info
            .read(path.to_str().unwrap(), DataType::Audio)
            .await
            .unwrap();
        assert_eq!(facts["durationMs"], expected, "{name}");
        assert!(facts["rawLocation"].is_null());
    }
    assert!(requests.try_recv().is_err());
}

#[tokio::test]
async fn unknown_audio_details_remain_nullable_without_restoring_a_host_parser() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("unknown.audio");
    std::fs::write(&path, b"unknown container").unwrap();
    let host = Arc::new(Host::default());
    let (_generation, mut requests) = host.connect();
    let info = Info::sdk(host);
    assert!(
        info.read(path.to_str().unwrap(), DataType::Audio)
            .await
            .unwrap()
            .is_null()
    );
    assert!(
        info.read(
            directory.path().join("missing").to_str().unwrap(),
            DataType::Audio
        )
        .await
        .is_err()
    );
    assert!(requests.try_recv().is_err());
}

#[tokio::test]
async fn sdk_video_details_use_shared_container_readers_without_whole_host_facts() {
    let host = Arc::new(Host::default());
    let (_generation, mut requests) = host.connect();
    let info = Info::sdk(host);
    for name in [
        "video-h264-baseline.mp4",
        "video-h264-high.mp4",
        "video-h265.mp4",
        "video-h265-hvc1.mp4",
        "video-vp8.webm",
        "video-vp8.mkv",
        "video-mjpeg.avi",
        "video-flv1.flv",
        "video-wmv2.wmv",
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../plain-rs/testdata")
            .join(name);
        let facts = info
            .read(path.to_str().unwrap(), DataType::Video)
            .await
            .unwrap();
        assert!(facts["width"].as_i64().unwrap() > 0, "{name}");
        assert!(facts["height"].as_i64().unwrap() > 0, "{name}");
        if name.starts_with("video-h264") {
            assert_eq!(facts["width"], 320, "{name}");
            assert_eq!(facts["height"], 240, "{name}");
            assert_eq!(facts["displayWidth"], 320, "{name}");
            assert_eq!(facts["displayHeight"], 240, "{name}");
        }
        assert!(facts["durationMs"].as_i64().unwrap() > 0, "{name}");
        assert!(facts["frameRate"].as_f64().unwrap() > 0.0, "{name}");
    }
    assert!(requests.try_recv().is_err());
}

#[tokio::test]
async fn opaque_video_details_use_one_owned_descriptor_without_sdk_metadata_or_staging() {
    for (name, width, height, duration) in [
        ("video-h264-baseline.mp4", 320, 240, 6000),
        ("video-vp8.webm", 16, 16, 120),
        ("video-vp8.mkv", 16, 16, 120),
        ("video-mjpeg.avi", 16, 16, 120),
        ("video-flv1.flv", 16, 16, 120),
        ("video-wmv2.wmv", 16, 16, 120),
        ("video-h264-no-metadata.flv", 52, 38, 0),
        ("video-flv1-no-metadata.flv", 16, 16, 0),
        ("video-screen-no-metadata.flv", 52, 38, 0),
        ("video-h264.ts", 52, 38, 0),
        ("video-mpeg2.ts", 64, 48, 0),
        ("video-hevc.ts", 96, 80, 0),
        ("video-hevc-no-metadata.flv", 96, 80, 0),
    ] {
        assert_owned_video_info(name, width, height, duration).await;
    }
}

async fn assert_owned_video_info(name: &str, width: i64, height: i64, duration: i64) {
    let directory = tempfile::tempdir().unwrap();
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../plain-rs/testdata")
            .join(name),
    )
    .unwrap();
    let mut wrapped = b"private-prefix".to_vec();
    let offset = wrapped.len() as i64;
    wrapped.extend_from_slice(&bytes);
    wrapped.extend(b"private-suffix");
    let source = directory.path().join("descriptor");
    std::fs::write(&source, wrapped).unwrap();
    let host = Arc::new(Host::default());
    let (generation, mut requests) = host.connect();
    let resources = Arc::new(plain_rs::host::resources::NativeResources::new(
        host.clone(),
    ));
    let mut info = Info::new(
        host.clone(),
        resources.clone(),
        directory.path().join("cache"),
    );
    Arc::get_mut(&mut info).unwrap().sdk = true;
    let reading =
        tokio::spawn(async move { info.read("content://fixture/video", DataType::Video).await });
    let request = requests.recv().await.unwrap();
    assert_eq!(request["method"], "fileResourceStream");
    let stream = resources
        .attach_native(request["params"]["id"].as_str().unwrap(), generation)
        .unwrap();
    host.reply(generation, json!({"id":request["id"],"result":null}))
        .unwrap();
    let packet: Value =
        serde_json::from_str(&stream.next_request().await.unwrap().unwrap()).unwrap();
    assert_eq!(
        packet,
        json!({"path":"content://fixture/video","file":true})
    );
    stream
        .file(
            plain_rs::host::resource_stream::NativeResourceFile::new(
                std::fs::File::open(source).unwrap(),
                offset,
                bytes.len() as i64,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let facts = reading.await.unwrap().unwrap();
    assert_eq!(
        (facts["width"].as_i64(), facts["height"].as_i64()),
        (Some(width), Some(height))
    );
    assert_eq!(facts["durationMs"], duration);
    assert!(requests.try_recv().is_err());
    assert!(!directory.path().join("cache").exists());
}
