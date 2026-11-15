use super::*;
use plain_libs::image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use std::{fs::File, io::Write};

fn fixture(format: ImageFormat, width: u32, height: u32) -> Vec<u8> {
    let image = RgbaImage::from_fn(width, height, |x, y| {
        Rgba([(x * 7) as u8, (y * 13) as u8, 93, ((x + y) * 17) as u8])
    });
    let mut bytes = std::io::Cursor::new(Vec::new());
    let image = DynamicImage::ImageRgba8(image);
    let image = if format == ImageFormat::Jpeg {
        DynamicImage::ImageRgb8(image.to_rgb8())
    } else {
        image
    };
    image.write_to(&mut bytes, format).unwrap();
    bytes.into_inner()
}

fn source(bytes: &[u8]) -> (tokio::sync::watch::Sender<bool>, SourceReader) {
    let mut file = plain_libs::temporary::tempfile().unwrap();
    file.write_all(b"before").unwrap();
    file.write_all(bytes).unwrap();
    file.write_all(b"after").unwrap();
    let (signal, stop) = tokio::sync::watch::channel(false);
    (
        signal,
        SourceReader {
            reader: plain_libs::utils::range_reader::RangeReader::new(file, 6, bytes.len() as u64)
                .unwrap(),
            stop,
        },
    )
}

#[test]
fn dimensions_preserve_formats_ranges_and_cancellation() {
    for format in [
        ImageFormat::Png,
        ImageFormat::Jpeg,
        ImageFormat::Gif,
        ImageFormat::WebP,
    ] {
        let bytes = fixture(format, 8, 4);
        let mut file = plain_libs::temporary::tempfile().unwrap();
        file.write_all(&bytes).unwrap();
        // The metadata reader receives a newly opened file at its original position.
        use std::io::{Seek, SeekFrom};
        file.seek(SeekFrom::Start(0)).unwrap();
        assert_eq!(file_dimensions(file), Some((8, 4)));
        let (signal, mut reader) = source(&bytes);
        assert_eq!(source_dimensions(&mut reader), Some((8, 4)));
        let (signal, mut reader) = {
            drop(signal);
            source(&bytes)
        };
        signal.send(true).unwrap();
        assert_eq!(source_dimensions(&mut reader), None);
        assert_eq!(reader.reader.stream_position().unwrap(), 0);
    }
}

#[test]
fn dimension_queries_preserve_limits_and_reject_damaged_headers() {
    for format in [
        ImageFormat::Png,
        ImageFormat::Jpeg,
        ImageFormat::Gif,
        ImageFormat::WebP,
    ] {
        let bytes = fixture(format, 8, 4);
        for (width, height, accepted) in [(8, 4, true), (7, 4, false), (8, 3, false)] {
            let mut reader = plain_libs::image::ImageReader::new(std::io::Cursor::new(&bytes))
                .with_guessed_format()
                .unwrap();
            let mut limits = plain_libs::image::Limits::default();
            limits.max_image_width = Some(width);
            limits.max_image_height = Some(height);
            reader.limits(limits);
            let result = reader.into_dimensions();
            if accepted {
                assert_eq!(result.unwrap(), (8, 4));
            } else {
                assert!(matches!(
                    result,
                    Err(plain_libs::image::ImageError::Limits(_))
                ));
            }
        }
        let mut reader = plain_libs::image::ImageReader::new(std::io::Cursor::new(&bytes))
            .with_guessed_format()
            .unwrap();
        reader.no_limits();
        assert_eq!(reader.into_dimensions().unwrap(), (8, 4));
        let truncated = &bytes[..bytes.len().min(8)];
        let mut reader = plain_libs::image::ImageReader::new(std::io::Cursor::new(truncated));
        reader.set_format(format);
        assert!(reader.into_dimensions().is_err());
    }
    let mut png = fixture(ImageFormat::Png, 8, 4);
    let mut reader = plain_libs::image::ImageReader::new(std::io::Cursor::new(&png))
        .with_guessed_format()
        .unwrap();
    let mut limits = plain_libs::image::Limits::default();
    limits.max_alloc = Some(0);
    reader.limits(limits);
    assert!(matches!(
        reader.into_dimensions(),
        Err(plain_libs::image::ImageError::Limits(_))
    ));
    png[16] ^= 1;
    let reader = plain_libs::image::ImageReader::new(std::io::Cursor::new(&png))
        .with_guessed_format()
        .unwrap();
    assert!(reader.into_dimensions().is_err());
}

#[test]
fn thumbnails_preserve_sizing_alpha_and_output_format() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    for (format, extension) in [
        (ImageFormat::Png, "png"),
        (ImageFormat::Jpeg, "jpg"),
        (ImageFormat::Gif, "gif"),
        (ImageFormat::WebP, "webp"),
    ] {
        let input = fixture(format, 8, 4);
        let path = directory.path().join(format!("image.{extension}"));
        std::fs::write(&path, &input).unwrap();
        for (width, height, expected) in [
            (2, 2, (2, 1)),
            (32, 32, (8, 4)),
            (0, 2, (4, 2)),
            (0, 0, (8, 4)),
        ] {
            let bytes = thumbnail(path.to_str().unwrap().into(), width, height)
                .unwrap()
                .unwrap();
            let image = plain_libs::image::load_from_memory(&bytes).unwrap();
            assert_eq!((image.width(), image.height()), expected);
            assert_eq!(
                plain_libs::image::guess_format(&bytes).unwrap(),
                if format == ImageFormat::Png {
                    ImageFormat::Png
                } else {
                    ImageFormat::Jpeg
                }
            );
            if format == ImageFormat::Png && expected == (8, 4) {
                assert_eq!(
                    image.to_rgba8(),
                    plain_libs::image::load_from_memory(&input)
                        .unwrap()
                        .to_rgba8()
                );
            }
        }
    }
}

#[test]
fn thumbnail_orientation_and_animated_first_frame_are_preserved() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let jpeg = fixture(ImageFormat::Jpeg, 3, 2);
    let exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
    let mut bytes = jpeg[..2].to_vec();
    bytes.extend([0xff, 0xe1]);
    bytes.extend(((exif.len() + 2) as u16).to_be_bytes());
    bytes.extend(exif);
    bytes.extend(&jpeg[2..]);
    let path = directory.path().join("rotated.jpg");
    std::fs::write(&path, bytes).unwrap();
    let bytes = thumbnail(path.to_str().unwrap().into(), 2, 3)
        .unwrap()
        .unwrap();
    let image = plain_libs::image::load_from_memory(&bytes).unwrap();
    assert_eq!((image.width(), image.height()), (2, 3));

    let path = directory.path().join("animated.gif");
    let mut bytes = Vec::new();
    {
        let mut encoder = plain_libs::image::codecs::gif::GifEncoder::new(&mut bytes);
        for color in [Rgba([220, 10, 10, 255]), Rgba([10, 10, 220, 255])] {
            encoder
                .encode_frame(plain_libs::image::Frame::new(RgbaImage::from_pixel(
                    8, 4, color,
                )))
                .unwrap();
        }
    }
    std::fs::write(&path, bytes).unwrap();
    let bytes = thumbnail(path.to_str().unwrap().into(), 8, 4)
        .unwrap()
        .unwrap();
    let image = plain_libs::image::load_from_memory(&bytes)
        .unwrap()
        .to_rgb8();
    assert!(image.pixels().all(|pixel| pixel[0] > 200 && pixel[2] < 30));
}

#[test]
fn conversion_preserves_pixels_and_failure_side_effects() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let input = directory.path().join("source.png");
    let output = directory.path().join("output.png");
    let bytes = fixture(ImageFormat::Png, 8, 4);
    std::fs::write(&input, &bytes).unwrap();
    let input = input.to_str().unwrap().to_owned();
    let output_name = output.to_str().unwrap().to_owned();
    assert_eq!(
        convert(input.clone(), output_name.clone()).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        plain_libs::image::open(&output).unwrap().to_rgba8(),
        plain_libs::image::load_from_memory(&bytes)
            .unwrap()
            .to_rgba8()
    );
    assert!(
        convert(
            input,
            directory
                .path()
                .join("missing/output.png")
                .to_str()
                .unwrap()
                .into()
        )
        .is_err()
    );
    for (name, bytes, unsupported) in [
        ("unsupported.bin", b"unknown".as_slice(), true),
        ("bad.png", b"\x89PNG".as_slice(), false),
    ] {
        let path = directory.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        std::fs::write(&output, b"preserved").unwrap();
        let result = convert(path.to_str().unwrap().into(), output_name.clone());
        let thumb = thumbnail(path.to_str().unwrap().into(), 8, 4);
        if unsupported {
            assert_eq!(result.unwrap(), Value::Bool(false));
            assert!(thumb.unwrap().is_none());
        } else {
            assert!(result.is_err());
            assert!(thumb.is_err());
        }
        assert_eq!(std::fs::read(&output).unwrap(), b"preserved");
    }
    let missing = directory
        .path()
        .join("missing.png")
        .to_str()
        .unwrap()
        .to_owned();
    assert!(thumbnail(missing.clone(), 8, 4).is_err());
    assert!(convert(missing, output_name).is_err());
}

#[cfg(unix)]
#[path = "image_io_bench.rs"]
mod benchmark;
