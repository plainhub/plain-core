use super::*;

#[test]
fn sdk_thumbnail_bytes_use_shared_short_edge_sizing_and_preserve_png_alpha() {
    use plain_libs::image::{self, DynamicImage, ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;

    for (source, expected) in [((341, 1024), (256, 768)), ((100, 300), (100, 300))] {
        let mut input = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            source.0,
            source.1,
            Rgba([20, 40, 80, 128]),
        ))
        .write_to(&mut input, ImageFormat::Png)
        .unwrap();
        let input = input.into_inner();
        let bytes =
            plain_libs::media::image_io::limit_thumbnail_short_edge(input.clone(), 256, 1024)
                .unwrap();
        if source == expected {
            assert_eq!(bytes, input);
        }
        let image = image::load_from_memory_with_format(&bytes, ImageFormat::Png)
            .unwrap()
            .into_rgba8();
        assert_eq!(image.dimensions(), expected);
        assert_eq!(image.get_pixel(0, 0)[3], 128);
    }
}

#[test]
fn local_photos_use_rust_for_both_preview_sizes_even_with_a_media_id() {
    for name in ["a.jpg", "a.png", "a.gif", "a.webp"] {
        for (width, short_edge) in [(512, Some(256)), (1024, None)] {
            let request = Request {
                path: std::env::temp_dir().join(name).to_string_lossy().into_owned(),
                width,
                height: width,
                center_crop: false,
                short_edge,
                file_id: "catalog-id".into(),
                access_order: None,
                file_name: name.into(),
            };
            assert!(uses_rust_still_decoder(&request));
            assert!(!uses_system_thumbnail(&request));
        }
    }
}

#[cfg(feature = "media")]
#[test]
fn local_still_formats_use_shared_rust_decoding_and_the_shared_cache_on_every_host() {
    let mut request = Request {
        path: std::env::temp_dir()
            .join("image.BMP")
            .to_string_lossy()
            .into_owned(),
        width: 256,
        height: 256,
        center_crop: true,
        short_edge: None,
        file_id: "catalog-id".into(),
        access_order: None,
        file_name: "image.BMP".into(),
    };
    for name in [
        "image.BMP",
        "image.tif",
        "image.TIFF",
        "image.avif",
        "image.heic",
        "image.HEIF",
    ] {
        request.path = std::env::temp_dir()
            .join(name)
            .to_string_lossy()
            .into_owned();
        request.file_name = name.into();
        assert!(uses_rust_still_decoder(&request), "{name}");
        assert!(!uses_system_thumbnail(&request), "{name}");
    }
    for path in [
        "content://images/one.bmp",
        "ph://one.bmp",
        "relative.bmp",
        "image.jpg",
    ] {
        request.path = path.into();
        assert!(!uses_rust_still_decoder(&request));
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[test]
fn native_catalog_ids_do_not_select_the_sdk_thumbnail_route() {
    let request = Request {
        path: "/image.jpg".into(),
        width: 80,
        height: 60,
        center_crop: true,
        short_edge: None,
        file_id: "catalog-id".into(),
        access_order: None,
        file_name: "image.jpg".into(),
    };
    assert!(!uses_system_thumbnail(&request));
}

#[cfg(all(
    not(feature = "runtime-media"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[tokio::test]
async fn core_only_thumbnail_preserves_aspect_without_host_requests_or_output_tokens() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let path = directory.path().join("image.png");
    plain_libs::image::RgbImage::from_pixel(8, 4, plain_libs::image::Rgb([200, 100, 50]))
        .save(&path)
        .unwrap();
    let core = crate::test_runtime::Core::open(&directory.path().join("core")).unwrap();
    let state = core.runtime_state();
    let (_generation, mut requests) = state.host.connect();
    let request = Request {
        path: path.to_string_lossy().into_owned(),
        width: 2,
        height: 2,
        center_crop: true,
        short_edge: None,
        file_id: "catalog-id".into(),
        access_order: None,
        file_name: "image.png".into(),
    };
    let bytes = state
        .thumbnails
        .get(&state, request)
        .await
        .unwrap()
        .unwrap();
    let image = plain_libs::image::load_from_memory(&bytes).unwrap();
    assert_eq!((image.width(), image.height()), (8, 4));
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}

#[cfg(not(feature = "runtime-media"))]
#[tokio::test]
async fn core_only_thumbnail_applies_exif_orientation_before_sizing() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let path = directory.path().join("rotated.jpg");
    let image = plain_libs::image::RgbImage::from_pixel(3, 2, plain_libs::image::Rgb([200, 100, 50]));
    let mut jpeg = Vec::new();
    plain_libs::image::codecs::jpeg::JpegEncoder::new(&mut jpeg)
        .encode_image(&image)
        .unwrap();
    let exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
    let mut bytes = jpeg[..2].to_vec();
    bytes.extend_from_slice(&[0xff, 0xe1]);
    bytes.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
    bytes.extend_from_slice(exif);
    bytes.extend_from_slice(&jpeg[2..]);
    std::fs::write(&path, bytes).unwrap();
    let request = Request {
        path: path.to_string_lossy().into_owned(),
        width: 2,
        height: 3,
        center_crop: false,
        short_edge: None,
        file_id: String::new(),
        access_order: None,
        file_name: "rotated.jpg".into(),
    };
    let image =
        plain_libs::image::load_from_memory(&native(&request, directory.path()).await.unwrap().unwrap())
            .unwrap();
    assert_eq!((image.width(), image.height()), (2, 3));
}

#[tokio::test]
async fn native_png_thumbnail_retains_transparent_and_partial_alpha() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let path = directory.path().join("transparent.png");
    plain_libs::image::RgbaImage::from_fn(768, 768, |x, _| {
        plain_libs::image::Rgba([40, 90, 150, if x < 384 { 0 } else { 128 }])
    })
    .save(&path)
    .unwrap();
    let request = Request {
        path: path.to_string_lossy().into_owned(),
        width: 256,
        height: 256,
        center_crop: true,
        short_edge: None,
        file_id: String::new(),
        access_order: None,
        file_name: "transparent.png".into(),
    };
    let bytes = native(&request, directory.path()).await.unwrap().unwrap();
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    let decoded = plain_libs::image::load_from_memory(&bytes).unwrap().into_rgba8();
    assert_eq!(decoded.dimensions(), (256, 256));
    assert_eq!(decoded.get_pixel(32, 128)[3], 0);
    assert_eq!(decoded.get_pixel(224, 128)[3], 128);
}

#[tokio::test]
async fn native_image_fit_preserves_aspect_without_enlarging_small_images() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let path = directory.path().join("small.png");
    plain_libs::image::RgbImage::from_pixel(8, 4, plain_libs::image::Rgb([200, 100, 50]))
        .save(&path)
        .unwrap();
    for (width, height, dimensions) in [(2, 2, (2, 1)), (32, 32, (8, 4))] {
        let mut first = None;
        for center_crop in [true, false] {
            let request = Request {
                path: path.to_string_lossy().into_owned(),
                width,
                height,
                center_crop,
                short_edge: None,
                file_id: String::new(),
                access_order: None,
                file_name: "small.png".into(),
            };
            let bytes = native(&request, directory.path()).await.unwrap().unwrap();
            let image = plain_libs::image::load_from_memory(&bytes).unwrap();
            assert_eq!((image.width(), image.height()), dimensions);
            if let Some(expected) = &first {
                assert_eq!(&bytes, expected);
            } else {
                first = Some(bytes);
            }
        }
    }
}
