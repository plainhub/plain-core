use super::*;

fn minimal_jpeg(w: u16, h: u16, progressive: bool) -> Vec<u8> {
    let mut v = vec![0xFF, 0xD8];
    // APP0/JFIF segment to exercise the skip path.
    v.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x04, 0x4A, 0x46]);
    let sof: u8 = if progressive { 0xC2 } else { 0xC0 };
    v.push(0xFF);
    v.push(sof);
    v.extend_from_slice(&[0x00, 0x11, 0x08]); // len, precision
    v.extend_from_slice(&h.to_be_bytes());
    v.extend_from_slice(&w.to_be_bytes());
    v.push(0x01); // 1 component
    v
}

fn minimal_png(w: u32, h: u32) -> Vec<u8> {
    let mut v = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    v.extend_from_slice(&13u32.to_be_bytes());
    v.extend_from_slice(b"IHDR");
    v.extend_from_slice(&w.to_be_bytes());
    v.extend_from_slice(&h.to_be_bytes());
    v.extend_from_slice(&[8, 6, 0, 0, 0]);
    v
}

#[test]
fn sniff_jpeg_baseline_and_progressive() {
    let s = sniff_header(&minimal_jpeg(4000, 3000, false)).unwrap();
    assert_eq!(
        s,
        Sniffed {
            kind: ImageKind::Jpeg,
            width: 4000,
            height: 3000,
            progressive: false,
            preserve_alpha: false,
            orientation: 1,
            work_bytes: 0,
        }
    );
    let s = sniff_header(&minimal_jpeg(640, 480, true)).unwrap();
    assert!(s.progressive);
}

#[test]
fn sniff_jpeg_fills_and_dqt() {
    // Fill bytes + a DQT segment before SOF must be walked correctly.
    let mut v = vec![0xFF, 0xD8, 0xFF, 0xFF, 0xFF];
    v.push(0xDB); // DQT
    v.extend_from_slice(&[0x00, 0x45]); // len 69
    v.extend_from_slice(&[0u8; 67]);
    v.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 0x08]);
    v.extend_from_slice(&3000u16.to_be_bytes());
    v.extend_from_slice(&4000u16.to_be_bytes());
    v.push(0x03);
    let s = sniff_header(&v).unwrap();
    assert_eq!((s.width, s.height), (4000, 3000));
}

#[test]
fn sniff_png_gif_bmp_webp_vp8x() {
    let s = sniff_header(&minimal_png(3840, 2880)).unwrap();
    assert_eq!((s.kind, s.width, s.height), (ImageKind::Png, 3840, 2880));

    let mut gif = b"GIF89a".to_vec();
    gif.extend_from_slice(&320u16.to_le_bytes());
    gif.extend_from_slice(&240u16.to_le_bytes());
    gif.extend_from_slice(&[0; 4]);
    let s = sniff_header(&gif).unwrap();
    assert_eq!((s.kind, s.width, s.height), (ImageKind::Gif, 320, 240));

    let mut bmp = vec![b'B', b'M'];
    bmp.extend_from_slice(&[0u8; 16]); // up to offset 18
    bmp.extend_from_slice(&800i32.to_le_bytes());
    bmp.extend_from_slice(&600i32.to_le_bytes());
    let s = sniff_header(&bmp).unwrap();
    assert_eq!((s.kind, s.width, s.height), (ImageKind::Bmp, 800, 600));
    assert!(!s.kind.can_passthrough());

    let mut payload = vec![0; 4];
    payload.extend_from_slice(&127u32.to_le_bytes()[..3]);
    payload.extend_from_slice(&63u32.to_le_bytes()[..3]);
    let webp = webp_chunk(b"VP8X", &payload);
    let s = sniff_header(&webp).unwrap();
    assert_eq!((s.kind, s.width, s.height), (ImageKind::WebP, 128, 64));
}

#[test]
fn sniff_errors_on_truncated_and_garbage() {
    assert_eq!(sniff_header(&[0xFF, 0xD8]), Err(SniffError::Truncated));
    assert_eq!(sniff_header(b"not an image"), Err(SniffError::Unknown));
    // JPEG marker walk that ends before SOF.
    let v = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00];
    assert_eq!(sniff_header(&v), Err(SniffError::Truncated));
    // PNG with zero dims.
    let bad = minimal_png(0, 10);
    assert_eq!(sniff_header(&bad), Err(SniffError::Unknown));
}

#[test]
fn sniff_real_bench_photos() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tmp-bench-media");
    for (name, w, h) in [
        ("photo-4000x3000.jpg", 4000u32, 3000u32),
        ("photo-4917x3456.jpg", 4917, 3456),
        ("photo-3840x2880.png", 3840, 2880),
    ] {
        let p = format!("{dir}/{name}");
        let Ok(mut f) = std::fs::File::open(&p) else {
            println!("SKIP sniff_real_bench_photos: missing {p}");
            continue;
        };
        use std::io::Read;
        let mut head = vec![0u8; 256 * 1024];
        let n = f.read(&mut head).unwrap_or(0);
        let s = sniff_header(&head[..n]).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!((s.width, s.height), (w, h), "{name}");
    }
}

#[test]
fn every_truncated_png_header_is_an_error_without_panicking() {
    let png = minimal_png(800, 600);
    for length in 12..24 {
        assert_eq!(sniff_header(&png[..length]), Err(SniffError::Truncated));
    }
}

fn webp_chunk(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let length = payload.len() as u32;
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&(12 + length + (length & 1)).to_le_bytes());
    bytes.extend_from_slice(b"WEBP");
    bytes.extend_from_slice(kind);
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.extend_from_slice(payload);
    if length & 1 != 0 {
        bytes.push(0);
    }
    bytes
}

#[test]
fn real_webp_encoder_outputs_keep_dimensions_and_transparency_flags() {
    for alpha in [false, true] {
        let source = image::RgbaImage::from_fn(37, 19, |x, _| {
            image::Rgba([40, 90, 150, if alpha && x < 20 { 128 } else { 255 }])
        });
        let mut bytes = Vec::new();
        if alpha {
            image::codecs::webp::WebPEncoder::new_lossless(&mut bytes)
                .encode(source.as_raw(), 37, 19, image::ExtendedColorType::Rgba8)
                .unwrap();
        } else {
            let rgb = image::DynamicImage::ImageRgba8(source.clone()).into_rgb8();
            image::codecs::webp::WebPEncoder::new_lossless(&mut bytes)
                .encode(rgb.as_raw(), 37, 19, image::ExtendedColorType::Rgb8)
                .unwrap();
        }
        let actual = sniff_header(&bytes).unwrap();
        assert_eq!(
            (actual.width, actual.height, actual.preserve_alpha),
            (37, 19, alpha)
        );
        assert_eq!(
            image::load_from_memory(&bytes).unwrap().into_rgba8(),
            source
        );
        assert_eq!(bytes.get(12..16), Some(b"VP8L".as_slice()));
        let mut header = vec![if alpha { 0x10 } else { 0 }, 0, 0, 0];
        header.extend_from_slice(&36_u32.to_le_bytes()[..3]);
        header.extend_from_slice(&18_u32.to_le_bytes()[..3]);
        let mut extended = webp_chunk(b"VP8X", &header);
        extended.extend_from_slice(&bytes[12..]);
        let length = (extended.len() - 8) as u32;
        extended[4..8].copy_from_slice(&length.to_le_bytes());
        let actual = sniff_header(&extended).unwrap();
        assert_eq!(
            (actual.width, actual.height, actual.preserve_alpha),
            (37, 19, alpha)
        );
        assert_eq!(
            image::load_from_memory(&extended).unwrap().into_rgba8(),
            source
        );
        for input in [&bytes, &extended] {
            assert_eq!(
                crate::utils::image_dimensions::dimensions(input),
                Some((37, 19))
            );
            #[cfg(feature = "filesystem")]
            {
                let directory = crate::temporary::tempdir().unwrap();
                let path = directory.path().join("static.webp");
                std::fs::write(&path, input).unwrap();
                assert!(!crate::filesystem::image_format::is_animated_image_or_svg(
                    &path
                ));
            }
        }
    }
}

#[test]
fn webp_prefixes_and_invalid_image_headers_are_rejected() {
    let extended = webp_chunk(b"VP8X", &[0x10, 0, 0, 0, 36, 0, 0, 18, 0, 0]);
    let packed = 36_u32 | (18 << 14) | (1 << 28);
    let mut payload = vec![0x2f];
    payload.extend_from_slice(&packed.to_le_bytes());
    let lossless = webp_chunk(b"VP8L", &payload);
    let lossy = webp_chunk(b"VP8 ", &[0, 0, 0, 0x9d, 0x01, 0x2a, 37, 0, 19, 0]);
    for (bytes, required) in [(&extended, 30), (&lossless, 25), (&lossy, 30)] {
        for end in 0..required {
            assert!(sniff_header(&bytes[..end]).is_err(), "prefix {end}");
        }
        assert_eq!(
            (
                sniff_header(bytes).unwrap().width,
                sniff_header(bytes).unwrap().height
            ),
            (37, 19)
        );
        let mut invalid = bytes.clone();
        invalid[4..8].copy_from_slice(&0_u32.to_le_bytes());
        assert_eq!(sniff_header(&invalid), Err(SniffError::Unknown));
        let mut invalid = bytes.clone();
        invalid[16..20].copy_from_slice(&1_u32.to_le_bytes());
        assert_eq!(sniff_header(&invalid), Err(SniffError::Unknown));
    }
    let mut invalid = lossless.clone();
    invalid[20] = 0;
    assert_eq!(sniff_header(&invalid), Err(SniffError::Unknown));
    let mut invalid = lossless;
    invalid[24] |= 0x20;
    assert_eq!(sniff_header(&invalid), Err(SniffError::Unknown));
    let mut invalid = lossy.clone();
    invalid[23] = 0;
    assert_eq!(sniff_header(&invalid), Err(SniffError::Unknown));
    let mut invalid = lossy;
    invalid[20] = 1;
    assert_eq!(sniff_header(&invalid), Err(SniffError::Unknown));
    assert_eq!(
        sniff_header(&webp_chunk(b"VP8X", &[0; 9])),
        Err(SniffError::Unknown)
    );
    assert_eq!(
        sniff_header(&webp_chunk(b"VP8L", &[0; 4])),
        Err(SniffError::Unknown)
    );
}
