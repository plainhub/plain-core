use super::*;
use std::io::Cursor;

#[test]
fn path_and_reader_preprocessing_produce_identical_tensors() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("photo.png");
    image::RgbImage::from_fn(96, 80, |x, y| image::Rgb([x as u8, y as u8, (x + y) as u8]))
        .save(&path)
        .unwrap();
    let bytes = std::fs::read(&path).unwrap();
    for resize in [Resize::Stretch, Resize::CenterCrop] {
        for layout in [Layout::Nchw, Layout::Nhwc] {
            let mut config = crate::inference::models::default_model::manifest().preprocess;
            config.resize = resize;
            config.layout = layout;
            let expected = tensor(&path, &config).unwrap().unwrap();
            let actual = tensor_reader(Cursor::new(&bytes), &config)
                .unwrap()
                .unwrap();
            assert_eq!(actual, expected);
        }
    }
}

#[test]
fn reader_preprocessing_keeps_tiny_and_unsupported_images_out_of_inference() {
    let config = crate::inference::models::default_model::manifest().preprocess;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("tiny.png");
    image::RgbImage::new(32, 32).save(&path).unwrap();
    assert!(
        tensor_reader(Cursor::new(std::fs::read(path).unwrap()), &config)
            .unwrap()
            .is_none()
    );
    assert!(
        tensor_reader(Cursor::new(b"not an image"), &config)
            .unwrap()
            .is_none()
    );
}
