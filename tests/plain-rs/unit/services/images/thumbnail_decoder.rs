use super::*;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[test]
fn native_catalog_ids_do_not_select_the_sdk_thumbnail_route() {
    let request = Request {
        path: "/image.jpg".into(),
        width: 80,
        height: 60,
        center_crop: true,
        file_id: "catalog-id".into(),
        file_name: "image.jpg".into(),
    };
    assert!(!uses_system_thumbnail(&request));
}

#[cfg(all(
    not(feature = "runtime-media"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[tokio::test]
async fn core_only_thumbnail_crops_without_host_requests_or_output_tokens() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("image.png");
    image::RgbImage::from_pixel(8, 4, image::Rgb([200, 100, 50]))
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
        file_id: "catalog-id".into(),
        file_name: "image.png".into(),
    };
    let bytes = state
        .thumbnails
        .get(&state, request)
        .await
        .unwrap()
        .unwrap();
    let image = image::load_from_memory(&bytes).unwrap();
    assert_eq!((image.width(), image.height()), (2, 2));
    assert!(requests.try_recv().is_err());
    core.stop_core().await;
}

#[cfg(not(feature = "runtime-media"))]
#[tokio::test]
async fn core_only_thumbnail_applies_exif_orientation_before_sizing() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("rotated.jpg");
    let image = image::RgbImage::from_pixel(3, 2, image::Rgb([200, 100, 50]));
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new(&mut jpeg)
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
        file_id: String::new(),
        file_name: "rotated.jpg".into(),
    };
    let image =
        image::load_from_memory(&native(&request, directory.path()).await.unwrap().unwrap())
            .unwrap();
    assert_eq!((image.width(), image.height()), (2, 3));
}
