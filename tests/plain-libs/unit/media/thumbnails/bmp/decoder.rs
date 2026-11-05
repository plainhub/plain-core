use super::*;
use std::io::Cursor;
#[path = "fixtures.rs"]
pub(super) mod fixtures;
use fixtures::{Counted, fixture};

fn decode(bytes: Vec<u8>) -> ImageResult<image::DynamicImage> {
    image::DynamicImage::from_decoder(BmpDecoder::new(Cursor::new(bytes))?)
}

#[test]
fn copied_decoder_preserves_bit_depth_palette_orientation_and_alpha() {
    let rgb = vec![255, 0, 0, 0, 255, 0, 0, 0, 255];
    for name in [
        "rgb32.bmp",
        "rgb88832.bmp",
        "rgb565.bmp",
        "rgb555.bmp",
        "palette2.bmp",
        "palette4.bmp",
        "palette8.bmp",
        "rle4.bmp",
        "rle8.bmp",
        "core24.bmp",
    ] {
        let image = decode(fixture(name)).unwrap();
        assert_eq!((image.width(), image.height()), (3, 1), "{name}");
        assert_eq!(image.to_rgb8().into_raw(), rgb, "{name}");
    }
    for name in ["rgb24.bmp", "rgb24-top-down.bmp"] {
        let image = decode(fixture(name)).unwrap();
        assert_eq!((image.width(), image.height()), (3, 2));
        let mut expected = rgb.clone();
        expected.extend_from_slice(&[0, 0, 0, 255, 255, 255, 255, 255, 0]);
        assert_eq!(image.to_rgb8().into_raw(), expected, "{name}");
    }
    assert_eq!(
        decode(fixture("palette1.bmp"))
            .unwrap()
            .to_rgb8()
            .into_raw(),
        [255, 0, 0, 0, 255, 0, 255, 0, 0]
    );
    assert_eq!(
        decode(fixture("alpha32.bmp"))
            .unwrap()
            .to_rgba8()
            .into_raw(),
        [255, 0, 0, 0, 0, 255, 0, 128, 0, 0, 255, 255]
    );
    for name in ["alpha32-v108.bmp", "alpha32-v124.bmp"] {
        assert_eq!(
            decode(fixture(name)).unwrap().to_rgba8().into_raw(),
            [255, 0, 0, 0, 0, 255, 0, 128, 0, 0, 255, 255]
        );
    }
    assert_eq!(
        decode(fixture("alpha16.bmp"))
            .unwrap()
            .to_rgba8()
            .into_raw(),
        [255, 0, 0, 0, 0, 255, 0, 136, 0, 0, 255, 255]
    );
}

#[test]
fn rle_runs_literals_deltas_and_row_ends_preserve_pixels() {
    let colors = [
        [255, 0, 0],
        [0, 255, 0],
        [0, 0, 255],
        [255, 255, 255],
        [0, 0, 0],
    ];
    for (name, indices) in [
        (
            "rle8-commands.bmp",
            vec![3, 3, 3, 3, 0, 1, 2, 3, 1, 1, 4, 2],
        ),
        (
            "rle8-delta.bmp",
            vec![3, 3, 3, 3, 4, 4, 2, 2, 4, 4, 4, 4, 0, 4, 4, 4],
        ),
        ("rle4-commands.bmp", vec![0, 3, 2, 0, 1, 2, 1, 2]),
    ] {
        let expected: Vec<u8> = indices
            .into_iter()
            .flat_map(|index| colors[index])
            .collect();
        assert_eq!(
            decode(fixture(name)).unwrap().to_rgb8().into_raw(),
            expected,
            "{name}"
        );
    }
}

#[test]
fn bitfield_expansion_matches_rounded_channel_values() {
    for bits in 1..=8 {
        let field = Bitfield::from_mask(((1u32 << bits) - 1) << 5, 32).unwrap();
        let max = (1u32 << bits) - 1;
        for value in 0..=max {
            assert_eq!(
                u32::from(field.read(value << 5)),
                (value * 255 + max / 2) / max
            );
        }
    }
}

#[test]
fn malformed_headers_and_every_truncation_return_errors_without_panics() {
    for name in [
        "rgb24.bmp",
        "alpha32.bmp",
        "alpha16.bmp",
        "rgb88832.bmp",
        "alpha32-v108.bmp",
        "alpha32-v124.bmp",
        "rgb565.bmp",
        "palette1.bmp",
        "palette2.bmp",
        "palette4.bmp",
        "palette8.bmp",
        "rle4.bmp",
        "rle8.bmp",
        "core24.bmp",
        "rle8-commands.bmp",
        "rle8-delta.bmp",
        "rle4-commands.bmp",
    ] {
        let valid = fixture(name);
        for length in 0..valid.len() {
            let result = std::panic::catch_unwind(|| decode(valid[..length].to_vec()));
            assert!(
                result.is_ok_and(|decoded| decoded.is_err()),
                "{name}: {length}"
            );
        }
    }
    for (offset, bytes) in [
        (0, b"XX".as_slice()),
        (14, &1u32.to_le_bytes()),
        (18, &(-1i32).to_le_bytes()),
        (18, &0i32.to_le_bytes()),
        (22, &0i32.to_le_bytes()),
        (22, &i32::MIN.to_le_bytes()),
        (26, &2u16.to_le_bytes()),
        (28, &9u16.to_le_bytes()),
        (30, &99u32.to_le_bytes()),
    ] {
        let mut corrupt = fixture("rgb24.bmp");
        corrupt[offset..offset + bytes.len()].copy_from_slice(bytes);
        assert!(decode(corrupt).is_err(), "offset {offset}");
    }
    let mut masks = fixture("alpha32.bmp");
    masks[54..58].fill(0);
    assert!(decode(masks).is_err());
    let mut masks = fixture("alpha32.bmp");
    masks[54..58].copy_from_slice(&5u32.to_le_bytes());
    assert!(decode(masks).is_err());
    let mut palette = fixture("palette1.bmp");
    palette[46..50].copy_from_slice(&256u32.to_le_bytes());
    assert!(decode(palette).is_err());
    for name in ["rle4.bmp", "rle8.bmp"] {
        let mut invalid = fixture(name);
        let offset = u32::from_le_bytes(invalid[10..14].try_into().unwrap()) as usize;
        invalid[offset..offset + 4].copy_from_slice(&[0, 2, 4, 0]);
        assert!(decode(invalid).is_err());
    }
    for height in [65_536i32, -65_536] {
        let mut bytes = fixture("rgb24.bmp");
        bytes[22..26].copy_from_slice(&height.to_le_bytes());
        assert!(BmpDecoder::new(Cursor::new(bytes)).is_err());
    }
    let mut oversized = fixture("rgb24.bmp");
    oversized[18..22].copy_from_slice(&65_535i32.to_le_bytes());
    oversized[22..26].copy_from_slice(&65_535i32.to_le_bytes());
    assert!(super::super::decode_reader(Cursor::new(oversized)).is_err());
}

#[tokio::test]
async fn duplicate_bmp_requests_generate_once_and_share_cached_bytes() {
    use crate::media::thumbnails::{self, ThumbOutcome, ThumbSpec};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("batch.bmp");
    let bytes = fixtures::expanded("rgb24.bmp", 1025, 769, false);
    std::fs::write(&path, &bytes).unwrap();
    let spec = ThumbSpec::sanitize(path.to_str().unwrap(), 128, 128, 75, 0, bytes.len() as i64);
    let key = thumbnails::thumb_cache_path(
        directory.path(),
        path.to_str().unwrap(),
        128,
        128,
        75,
        0,
        bytes.len() as i64,
    );
    let mut pending = Vec::new();
    for _ in 0..16 {
        let spec = spec.clone();
        let cache = directory.path().to_path_buf();
        pending.push(tokio::spawn(async move {
            thumbnails::get_thumbnail(spec, &cache).await.unwrap()
        }));
    }
    let mut results = Vec::new();
    for pending in pending {
        let ThumbOutcome::Generated(bytes) = pending.await.unwrap() else {
            panic!("expected encoded BMP thumbnail")
        };
        results.push(bytes);
    }
    for bytes in &results {
        assert_eq!(bytes, &results[0]);
        assert_eq!(bytes.as_ptr(), results[0].as_ptr());
    }
    assert_eq!(thumbnails::generations_for(&key.to_string_lossy()), 1);
}

#[tokio::test]
async fn bmp_preview_cache_preserves_formats_alpha_and_real_file_extensions() {
    let cache = tempfile::tempdir().unwrap();
    for &(name, alpha) in fixtures::FORMATS {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../plain-rs/testdata/bmp")
            .join(name);
        let original = decode(fixture(name)).unwrap();
        let first =
            crate::media::thumbnails::render(path.to_str().unwrap(), 96, 96, false, cache.path())
                .await
                .unwrap()
                .unwrap();
        let preview = image::load_from_memory(&first).unwrap();
        assert_eq!(
            (preview.width(), preview.height()),
            (original.width(), original.height()),
            "{name}"
        );
        if alpha {
            assert!(first.starts_with(b"\x89PNG\r\n\x1a\n"), "{name}");
            assert_eq!(preview.to_rgba8(), original.to_rgba8(), "{name}");
        } else {
            assert!(first.starts_with(&[0xff, 0xd8]), "{name}");
        }
        let second =
            crate::media::thumbnails::render(path.to_str().unwrap(), 96, 96, false, cache.path())
                .await
                .unwrap()
                .unwrap();
        assert_eq!(first, second, "{name}");
        assert_eq!(first.as_ptr(), second.as_ptr(), "{name}");
    }
    let mut directories = vec![cache.path().to_path_buf()];
    let mut files = 0;
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                directories.push(path);
            } else {
                let bytes = std::fs::read(&path).unwrap();
                assert_eq!(
                    path.extension().unwrap(),
                    if bytes.starts_with(b"\x89PNG") {
                        "png"
                    } else {
                        "jpg"
                    }
                );
                files += 1;
            }
        }
    }
    assert_eq!(files, fixtures::FORMATS.len());
}

#[test]
fn byte_rgb_decoding_reads_rows_instead_of_individual_pixels() {
    for name in ["rgb24.bmp", "rgb32.bmp", "alpha32.bmp"] {
        let mut input = Counted {
            input: Cursor::new(fixture(name)),
            reads: 0,
            bytes: 0,
            largest_read: 0,
        };
        let decoder = BmpDecoder::new(&mut input).unwrap();
        let height = decoder.dimensions().1;
        let before = decoder.reader.reads;
        image::DynamicImage::from_decoder(decoder).unwrap();
        assert_eq!(input.reads - before, height as usize, "{name}");
    }
}
