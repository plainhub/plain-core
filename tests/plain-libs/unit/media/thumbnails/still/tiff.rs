use super::*;
use std::{
    io::{Cursor, Write},
    ops::Range,
};

struct Fixture {
    bytes: Vec<u8>,
    chunks: Vec<Range<usize>>,
}

fn fixture(
    width: u32,
    height: u32,
    depth: u16,
    samples: u16,
    photo: u16,
    compression: u16,
    tile: Option<(u32, u32)>,
    rows: u32,
    extra: Vec<(u16, u16, Vec<u8>)>,
    chunks: Vec<Vec<u8>>,
) -> Fixture {
    let short = |v: u16| v.to_le_bytes().to_vec();
    let long = |v: u32| v.to_le_bytes().to_vec();
    let offsets = if tile.is_some() { 324 } else { 273 };
    let counts = if tile.is_some() { 325 } else { 279 };
    let mut tags = vec![
        (256, 4, long(width)),
        (257, 4, long(height)),
        (258, 3, (0..samples).flat_map(|_| short(depth)).collect()),
        (259, 3, short(compression)),
        (262, 3, short(photo)),
        (277, 3, short(samples)),
        (284, 3, short(1)),
        (offsets, 4, vec![0; chunks.len() * 4]),
        (
            counts,
            4,
            chunks.iter().flat_map(|v| long(v.len() as u32)).collect(),
        ),
    ];
    if let Some((w, h)) = tile {
        tags.extend([(322, 4, long(w)), (323, 4, long(h))]);
    } else {
        tags.push((278, 4, long(rows)));
    }
    for tag in extra {
        if let Some(existing) = tags.iter_mut().find(|t| t.0 == tag.0) {
            *existing = tag;
        } else {
            tags.push(tag);
        }
    }
    tags.sort_by_key(|t| t.0);
    let first = 8 + 2 + tags.len() * 12 + 4;
    let mut payload = first
        + tags
            .iter()
            .filter(|t| t.2.len() > 4)
            .map(|t| (t.2.len() + 1) & !1)
            .sum::<usize>();
    let mut ranges = Vec::new();
    for chunk in &chunks {
        ranges.push(payload..payload + chunk.len());
        payload += chunk.len();
    }
    tags.iter_mut().find(|t| t.0 == offsets).unwrap().2 =
        ranges.iter().flat_map(|r| long(r.start as u32)).collect();
    let mut bytes = b"II\x2a\0\x08\0\0\0".to_vec();
    bytes.extend_from_slice(&(tags.len() as u16).to_le_bytes());
    bytes.resize(first, 0);
    for (index, (tag, ty, data)) in tags.into_iter().enumerate() {
        let p = 10 + index * 12;
        bytes[p..p + 2].copy_from_slice(&tag.to_le_bytes());
        bytes[p + 2..p + 4].copy_from_slice(&ty.to_le_bytes());
        bytes[p + 4..p + 8].copy_from_slice(
            &(data.len() / if ty == 3 { 2 } else { 4 } as usize).to_le_bytes()[..4],
        );
        if data.len() <= 4 {
            bytes[p + 8..p + 8 + data.len()].copy_from_slice(&data);
        } else {
            let at = bytes.len() as u32;
            bytes[p + 8..p + 12].copy_from_slice(&at.to_le_bytes());
            bytes.extend_from_slice(&data);
            if bytes.len() % 2 != 0 {
                bytes.push(0);
            }
        }
    }
    for chunk in chunks {
        bytes.extend(chunk);
    }
    Fixture {
        bytes,
        chunks: ranges,
    }
}

fn rgb(x: u32, y: u32) -> [u8; 3] {
    [
        (x.wrapping_mul(11) % 251) as u8,
        (y.wrapping_mul(7) % 241) as u8,
        ((x + y).wrapping_mul(3) % 239) as u8,
    ]
}
fn compressed(data: &[u8], kind: u16) -> Vec<u8> {
    match kind {
        1 => data.to_vec(),
        5 => weezl::encode::Encoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8)
            .encode(data)
            .unwrap(),
        8 => {
            let mut encoder =
                flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
            encoder.write_all(data).unwrap();
            encoder.finish().unwrap()
        }
        32773 => {
            let mut output = Vec::new();
            for run in data.chunks(128) {
                output.push((run.len() - 1) as u8);
                output.extend_from_slice(run);
            }
            output
        }
        _ => unreachable!(),
    }
}
fn rgb_fixture(w: u32, h: u32, compression: u16, tile: Option<(u32, u32)>, alpha: bool) -> Fixture {
    let (cw, ch) = tile.unwrap_or((w, 16));
    let mut chunks = Vec::new();
    for ty in 0..h.div_ceil(ch) {
        for tx in 0..w.div_ceil(cw) {
            let rows = if tile.is_some() {
                ch
            } else {
                ch.min(h - ty * ch)
            };
            let mut chunk = Vec::new();
            for y in 0..rows {
                for x in 0..cw {
                    chunk.extend_from_slice(&rgb(tx * cw + x, ty * ch + y));
                    if alpha {
                        chunk.push(((x * 13 + y * 17) % 256) as u8);
                    }
                }
            }
            chunks.push(compressed(&chunk, compression));
        }
    }
    fixture(
        w,
        h,
        8,
        if alpha { 4 } else { 3 },
        2,
        compression,
        tile,
        ch,
        if alpha {
            vec![(338, 3, 2_u16.to_le_bytes().to_vec())]
        } else {
            Vec::new()
        },
        chunks,
    )
}

#[test]
fn compressed_strips_and_tiles_match_reference_pixels_and_box_filter() {
    for compression in [1, 5, 8, 32773] {
        for tile in [None, Some((32, 16))] {
            for alpha in [false, true] {
                let source = rgb_fixture(129, 67, compression, tile, alpha);
                let full = decode(Cursor::new(&source.bytes)).unwrap();
                let factor = (129_u32 / 11).min(67 / 7).clamp(1, 64);
                let expected = super::super::super::scale::box_downscale(&full, factor);
                let actual = thumbnail(Cursor::new(&source.bytes), 11, 7).unwrap();
                assert_eq!(
                    actual.raw(),
                    expected.raw(),
                    "compression={compression} tile={tile:?} alpha={alpha}"
                );
            }
        }
    }
}

#[test]
fn grayscale_palette_float_and_planar_samples_are_converted() {
    for depth in [1, 2, 4, 8] {
        let max = (1_u16 << depth) - 1;
        let width = 16;
        let mut pixels = vec![0; (width * depth as usize).div_ceil(8)];
        for x in 0..width {
            let v = x as u16 & max;
            let bit = x * depth as usize;
            pixels[bit / 8] |= (v as u8) << (8 - depth as usize - bit % 8);
        }
        let f = fixture(
            width as u32,
            1,
            depth,
            1,
            1,
            1,
            None,
            1,
            Vec::new(),
            vec![pixels.clone()],
        );
        let actual = decode(Cursor::new(f.bytes)).unwrap();
        assert_eq!(actual.raw().0[3..6], [(255 / max) as u8; 3]);
        let n = 1_usize << depth;
        let map = (0..3)
            .flat_map(|channel| {
                (0..n).flat_map(move |v| {
                    if channel == 0 {
                        ((v * 65535) / (n - 1)) as u16
                    } else {
                        0
                    }
                    .to_le_bytes()
                })
            })
            .collect();
        let f = fixture(
            width as u32,
            1,
            depth,
            1,
            3,
            1,
            None,
            1,
            vec![(320, 3, map)],
            vec![pixels],
        );
        let actual = decode(Cursor::new(f.bytes)).unwrap();
        assert_eq!(
            &actual.raw().0[3..6],
            &[(65535 / usize::from(max) >> 8) as u8, 0, 0]
        );
    }
    let f = fixture(
        2,
        1,
        16,
        3,
        2,
        1,
        None,
        1,
        Vec::new(),
        vec![
            [0_u16, 65535, 0, 65535, 0, 65535]
                .into_iter()
                .flat_map(u16::to_le_bytes)
                .collect(),
        ],
    );
    assert_eq!(
        decode(Cursor::new(f.bytes)).unwrap().raw().0,
        [0, 255, 0, 255, 0, 255]
    );
    let f = fixture(
        2,
        1,
        32,
        3,
        2,
        1,
        None,
        1,
        vec![(
            339,
            3,
            [3_u16; 3].into_iter().flat_map(u16::to_le_bytes).collect(),
        )],
        vec![
            [0_f32, 1., 0.5, 1., 0., f32::NAN]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect(),
        ],
    );
    assert_eq!(
        decode(Cursor::new(f.bytes)).unwrap().raw().0,
        [0, 255, 128, 255, 0, 0]
    );
    let f = fixture(
        2,
        1,
        8,
        3,
        2,
        1,
        None,
        1,
        vec![(284, 3, 2_u16.to_le_bytes().to_vec())],
        vec![vec![10, 20], vec![30, 40], vec![50, 60]],
    );
    assert_eq!(
        decode(Cursor::new(f.bytes)).unwrap().raw().0,
        [10, 30, 50, 20, 40, 60]
    );
}

#[test]
fn malformed_headers_truncated_chunks_and_dimension_bombs_are_refused() {
    let f = rgb_fixture(3, 2, 5, None, false);
    for end in 0..f.bytes.len() {
        assert!(
            decode(Cursor::new(&f.bytes[..end])).is_err(),
            "prefix {end}"
        );
    }
    let f = fixture(
        1_000_000,
        1_000_000,
        8,
        3,
        2,
        1,
        None,
        1,
        Vec::new(),
        vec![Vec::new()],
    );
    assert!(decode(Cursor::new(f.bytes)).is_err());
}

#[test]
fn jpeg_compressed_ycbcr_tiff_matches_jpeg_colors() {
    let source = image::RgbImage::from_fn(64, 48, |x, y| image::Rgb(rgb(x, y)));
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 90)
        .encode_image(&source)
        .unwrap();
    let f = fixture(64, 48, 8, 3, 6, 7, None, 48, vec![], vec![jpeg.clone()]);
    let actual = decode(Cursor::new(f.bytes)).unwrap();
    let expected = image::load_from_memory(&jpeg).unwrap().into_rgb8();
    assert_eq!((actual.width(), actual.height()), expected.dimensions());
    for (a, b) in actual.raw().0.iter().zip(expected.as_raw()) {
        assert!(a.abs_diff(*b) <= 2, "{a} {b}");
    }
}

#[test]
fn streamed_tiff_reads_each_compressed_pixel_chunk_once() {
    use std::sync::{Arc, Mutex};
    struct Counted {
        input: Cursor<Vec<u8>>,
        ranges: Arc<Mutex<Vec<Range<usize>>>>,
    }
    impl Read for Counted {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            let start = self.input.position() as usize;
            let n = self.input.read(out)?;
            self.ranges.lock().unwrap().push(start..start + n);
            Ok(n)
        }
    }
    impl Seek for Counted {
        fn seek(&mut self, p: std::io::SeekFrom) -> std::io::Result<u64> {
            self.input.seek(p)
        }
    }
    for tile in [None, Some((32, 32))] {
        let source = rgb_fixture(128, 1024, 8, tile, false);
        let chunks = source.chunks;
        let ranges = Arc::new(Mutex::new(Vec::new()));
        let actual = thumbnail(
            Counted {
                input: Cursor::new(source.bytes),
                ranges: ranges.clone(),
            },
            4,
            32,
        )
        .unwrap();
        assert_eq!((actual.width(), actual.height()), (4, 32));
        for chunk in chunks {
            let bytes = ranges
                .lock()
                .unwrap()
                .iter()
                .map(|r| {
                    r.end
                        .min(chunk.end)
                        .saturating_sub(r.start.max(chunk.start))
                })
                .sum::<usize>();
            assert_eq!(bytes, chunk.len(), "pixel chunk was skipped or reread");
        }
    }
}

#[test]
fn oversized_tile_memory_estimates_return_errors_without_panicking() {
    let source = fixture(
        1,
        1,
        8,
        3,
        2,
        1,
        Some((u32::MAX, u32::MAX)),
        1,
        vec![],
        vec![vec![0, 0, 0]],
    );
    let result = std::panic::catch_unwind(|| probe(Cursor::new(source.bytes)));
    assert!(result.is_ok(), "tile memory arithmetic panicked");
    assert!(
        result.unwrap().is_err(),
        "overflowed tile estimate was accepted"
    );
    let source = fixture(
        u32::MAX,
        u32::MAX,
        16,
        3,
        2,
        1,
        None,
        u32::MAX,
        vec![],
        vec![vec![0; 6]],
    );
    let result = std::panic::catch_unwind(|| probe(Cursor::new(source.bytes)));
    assert!(result.is_ok(), "full image memory arithmetic panicked");
    assert!(
        result.unwrap().is_err(),
        "overflowed full image estimate was accepted"
    );
}

#[test]
fn associated_cmyk_alpha_is_removed_before_rgb_conversion() {
    for (alpha_kind, pixels, expected) in [
        (
            1,
            vec![128, 0, 0, 0, 128, 64, 0, 0, 32, 128, 0, 0, 0, 0, 0],
            vec![0, 255, 255, 128, 95, 191, 191, 128, 0, 0, 0, 0],
        ),
        (
            2,
            vec![255, 0, 0, 0, 128, 128, 0, 0, 64, 128, 0, 0, 0, 0, 0],
            vec![0, 255, 255, 128, 95, 191, 191, 128, 255, 255, 255, 0],
        ),
    ] {
        let source = fixture(
            3,
            1,
            8,
            5,
            5,
            1,
            None,
            1,
            vec![(338, 3, u16::to_le_bytes(alpha_kind).to_vec())],
            vec![pixels],
        );
        let facts = probe(Cursor::new(&source.bytes)).unwrap();
        assert!(facts.preserve_alpha);
        let actual = decode(Cursor::new(&source.bytes)).unwrap();
        assert_eq!(
            actual.raw(),
            (expected.as_slice(), 3, 1),
            "ExtraSamples={alpha_kind}"
        );
        assert_eq!(
            thumbnail(Cursor::new(source.bytes), 3, 1).unwrap().raw(),
            actual.raw()
        );
    }
}

#[test]
fn ordinary_single_strip_tiff_fits_the_shared_memory_budget() {
    // Only metadata admission is exercised; the declared 12 MP raster is not decoded.
    for compression in [1, 5, 8, 32773] {
        let source = fixture(
            4000,
            3000,
            8,
            3,
            2,
            compression,
            None,
            3000,
            vec![],
            vec![vec![]],
        );
        let facts = probe(Cursor::new(source.bytes)).unwrap();
        assert_eq!(facts.work_bytes, 72_048_000, "compression {compression}");
        let cost =
            super::super::super::estimated_decoded_bytes(facts, 256, 192) * 2 + 256 * 256 * 4;
        assert_eq!(cost, 144_785_344);
        assert!(cost < 256 * 1024 * 1024);
    }
}

#[test]
fn jpeg_tiff_admission_includes_codec_planes_and_compressed_chunk_storage() {
    // JPEG payload bytes stand in for the compressed allocation; this test only probes metadata.
    for (photo, expected) in [
        (2, 72_048_000 + 12_000_000 * 8 + 17 * 2),
        (6, 12_000_000 * 16 + 17 * 2),
    ] {
        let source = fixture(
            4000,
            3000,
            8,
            3,
            photo,
            7,
            None,
            3000,
            vec![],
            vec![vec![0; 17]],
        );
        let facts = probe(Cursor::new(source.bytes)).unwrap();
        assert_eq!(facts.work_bytes, expected, "photometric {photo}");
    }
    let source = fixture(1, 1, 8, 3, 6, 7, None, 1, vec![], vec![vec![0; 131072]]);
    assert_eq!(
        probe(Cursor::new(source.bytes)).unwrap().work_bytes,
        16 + 131072 * 2
    );
}
