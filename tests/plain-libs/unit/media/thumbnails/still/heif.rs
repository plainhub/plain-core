use super::*;
use std::{collections::BTreeMap, io::Cursor, sync::Mutex};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../plain-rs/testdata/still-images")
            .join(name),
    )
    .unwrap()
}

#[test]
fn avif_and_heic_thumbnail_decode_matches_full_pixels_without_repeated_items() {
    for name in [
        "rgb.avif",
        "grid.avif",
        "rgb.heic",
        "grid.heic",
        "wide.heic",
    ] {
        let bytes = fixture(name);
        let ctx = Context::open(&bytes).unwrap();
        let properties = ctx.props(ctx.meta.primary).unwrap();
        let calls = Mutex::new(BTreeMap::<u32, usize>::new());
        let actual = decode_primary(&ctx, orientation(&properties).is_none(), &|ctx, id, p| {
            *calls.lock().unwrap().entry(id).or_default() += 1;
            coded(ctx, id, p)
        })
        .unwrap();
        assert_eq!(
            actual.raw(),
            decode_unoriented(&bytes).unwrap().raw(),
            "{name}"
        );
        assert!(
            calls.lock().unwrap().values().all(|n| *n == 1),
            "{name}: coded item decoded repeatedly"
        );
        if let Some((_, ids)) = ctx.grid(ctx.meta.primary).unwrap() {
            assert_eq!(calls.lock().unwrap().len(), ids.len(), "{name}");
        }
    }
}

#[test]
fn auxiliary_alpha_survives_thumbnail_decode() {
    for name in ["alpha.avif", "alpha.heic"] {
        let bytes = fixture(name);
        let image = decode(&bytes).unwrap();
        assert_eq!(image.channels(), 4, "{name}");
        assert!(image.raw().0.chunks_exact(4).any(|p| p[3] < 255), "{name}");
        let ctx = Context::open(&bytes).unwrap();
        let properties = ctx.props(ctx.meta.primary).unwrap();
        let expected = decode_primary(&ctx, orientation(&properties).is_none(), &coded).unwrap();
        assert_eq!(
            decode_unoriented(&bytes).unwrap().raw(),
            expected.raw(),
            "{name}"
        );
    }
}

#[test]
fn metadata_skips_pixel_payload_and_rejects_invalid_box_extents() {
    struct Counted {
        inner: Cursor<Vec<u8>>,
        read: usize,
    }
    impl Read for Counted {
        fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
            let n = self.inner.read(b)?;
            self.read += n;
            Ok(n)
        }
    }
    impl Seek for Counted {
        fn seek(&mut self, p: SeekFrom) -> std::io::Result<u64> {
            self.inner.seek(p)
        }
    }
    for name in ["grid.avif", "grid.heic"] {
        let bytes = fixture(name);
        let mut reader = Counted {
            inner: Cursor::new(bytes.clone()),
            read: 0,
        };
        let meta = metadata(&mut reader).unwrap();
        Context::open(&meta).unwrap();
        assert!(
            reader.read < bytes.len() / 2,
            "{name}: read the pixel payload"
        );
        for n in [0, 1, 7, 15] {
            assert!(
                metadata(&mut Cursor::new(&bytes[..n])).is_err(),
                "{name} {n}"
            );
        }
        let mut invalid = bytes;
        invalid[..4].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(metadata(&mut Cursor::new(invalid)).is_err());
    }
}

#[test]
fn rotations_and_mirrors_match_the_pixel_transform_contract() {
    use heic_rs::props::{
        Transform,
        simple::{Mirror, Rotation},
    };
    for transform in [
        Transform::Rotate(Rotation::Ccw90),
        Transform::Rotate(Rotation::Ccw180),
        Transform::Rotate(Rotation::Ccw270),
        Transform::Mirror(Mirror::LeftRight),
        Transform::Mirror(Mirror::TopBottom),
    ] {
        let mut properties = ItemProps::default();
        properties.transforms.push(transform);
        let o = orientation(&properties).unwrap();
        let data: Vec<u8> = (0..18).collect();
        let bitmap = Bitmap::new_rgb(data.clone(), 3, 2)
            .apply_orientation(super::super::super::exif::Orientation(o));
        let image = heic_rs::Image {
            data,
            width: 3,
            height: 2,
            layout: heic_rs::PixelLayout::Rgb8,
        };
        let expected = heic_rs::transform::apply_all(image, &properties.transforms).unwrap();
        assert_eq!(
            bitmap.raw(),
            (expected.data.as_slice(), expected.width, expected.height)
        );
    }
}

#[test]
fn hostile_hevc_regressions_return_without_panicking() {
    for n in 2..=6 {
        let bytes = fixture(&format!("hostile/bug{n}.heic"));
        let result = std::panic::catch_unwind(|| decode(&bytes));
        assert!(result.is_ok(), "HEVC bug {n} panicked");
        if matches!(n, 2 | 3 | 5) {
            assert!(result.unwrap().is_err(), "HEVC bug {n} was accepted");
        }
    }
}

#[test]
fn inconsistent_grid_dimensions_are_refused_before_any_tile_decode() {
    let mut bytes = fixture("grid.heic");
    let ctx = Context::open(&bytes).unwrap();
    assert!(ctx.grid(ctx.meta.primary).unwrap().is_some());
    let property = ctx
        .meta
        .props
        .associations(ctx.meta.primary)
        .iter()
        .find_map(|association| {
            ctx.meta
                .props
                .resolve(*association)
                .filter(|b| b.boxtype == *b"ispe")
        })
        .unwrap();
    let offset = property.payload.as_ptr() as usize - bytes.as_ptr() as usize;
    let width = ctx.props(ctx.meta.primary).unwrap().ispe.unwrap().width;
    drop(ctx);
    bytes[offset + 4..offset + 8].copy_from_slice(&(width + 1).to_be_bytes());
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let ctx = Context::open(&bytes).unwrap();
    let result = decode_primary(&ctx, false, &|ctx, id, p| {
        calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        coded(ctx, id, p)
    });
    assert!(result.is_err());
    assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 0);
}

#[test]
fn truncated_av1_items_are_rejected_without_panicking() {
    let bytes = fixture("rgb.avif");
    for length in [0, 1, 7, 15, bytes.len() / 2, bytes.len() - 1] {
        let result = std::panic::catch_unwind(|| decode(&bytes[..length]));
        assert!(result.is_ok(), "AVIF prefix {length} panicked");
        assert!(result.unwrap().is_err(), "AVIF prefix {length} accepted");
    }
}

#[test]
fn grid_admission_counts_the_canvas_once_and_fits_the_shared_budget() {
    let mut bytes = fixture("grid.heic");
    let ctx = Context::open(&bytes).unwrap();
    let property = ctx
        .meta
        .props
        .associations(ctx.meta.primary)
        .iter()
        .find_map(|association| {
            ctx.meta
                .props
                .resolve(*association)
                .filter(|b| b.boxtype == *b"ispe")
        })
        .unwrap();
    let offset = property.payload.as_ptr() as usize - bytes.as_ptr() as usize;
    drop(ctx);
    bytes[offset + 4..offset + 8].copy_from_slice(&4032_u32.to_be_bytes());
    bytes[offset + 8..offset + 12].copy_from_slice(&3024_u32.to_be_bytes());
    let directory = crate::temporary::tempdir().unwrap();
    let path = directory.path().join("metadata.heic");
    std::fs::write(&path, bytes).unwrap();
    let facts = probe(&path, ImageKind::Heif).unwrap();
    assert_eq!((facts.width, facts.height), (4032, 3024));
    assert_eq!(facts.work_bytes, 4032_u64 * 3024 * 16);
    let estimate = super::super::super::estimated_decoded_bytes(facts, 256, 192) * 2
        + 256 * 256 * 4
        + std::fs::metadata(path).unwrap().len();
    assert!(estimate < 512 * 1024 * 1024);
}

#[test]
fn av1_dimensions_must_match_container_ispe() {
    let mut bytes = fixture("rgb.avif");
    let ctx = Context::open(&bytes).unwrap();
    let property = ctx
        .meta
        .props
        .associations(ctx.meta.primary)
        .iter()
        .find_map(|association| {
            ctx.meta
                .props
                .resolve(*association)
                .filter(|b| b.boxtype == *b"ispe")
        })
        .unwrap();
    let offset = property.payload.as_ptr() as usize - bytes.as_ptr() as usize;
    let width = ctx.props(ctx.meta.primary).unwrap().ispe.unwrap().width;
    drop(ctx);
    bytes[offset + 4..offset + 8].copy_from_slice(&(width + 1).to_be_bytes());
    let result = std::panic::catch_unwind(|| decode_unoriented(&bytes));
    assert!(result.is_ok());
    assert!(result.unwrap().is_err());
}

#[test]
fn coded_tile_admission_prices_every_padded_item_once() {
    let directory = crate::temporary::tempdir().unwrap();
    let path = directory.path().join("grid.heic");
    let bytes = fixture("grid.heic");
    let ctx = Context::open(&bytes).unwrap();
    assert_eq!(ctx.grid(ctx.meta.primary).unwrap().unwrap().1.len(), 4);
    std::fs::write(&path, &bytes).unwrap();
    let facts = probe(&path, ImageKind::Heif).unwrap();
    assert_eq!((facts.width, facts.height), (1024, 768));
    assert_eq!(facts.work_bytes, 4_u64 * 512 * 512 * 16);
}

#[test]
fn hevc_without_clean_aperture_rejects_dimensions_that_disagree_with_ispe() {
    let bytes = fixture("grid.heic");
    let ctx = Context::open(&bytes).unwrap();
    let (_, ids) = ctx.grid(ctx.meta.primary).unwrap().unwrap();
    let id = ids[0];
    let mut properties = ctx.props(id).unwrap();
    assert!(
        !properties
            .transforms
            .iter()
            .any(|t| matches!(t, heic_rs::props::Transform::Crop(_)))
    );
    properties.ispe.as_mut().unwrap().width += 1;
    assert!(coded(&ctx, id, &properties).is_err());
    assert!(decode_unoriented(&fixture("rgb.heic")).is_ok());
}
