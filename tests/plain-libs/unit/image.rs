use super::*;
use std::io::Cursor;
const PNG: &[u8] = include_bytes!("../../../plain-rs/testdata/extracted-codecs/colors.png");
const WEBP: &[u8] = include_bytes!("../../../plain-rs/testdata/extracted-codecs/colors.webp");
const GIF: &[u8] = include_bytes!("../../../plain-rs/testdata/extracted-codecs/colors.gif");
#[test]
fn independent_png_webp_and_gif_fixtures_keep_pixels_and_dimensions() {
    let expected = [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];
    for bytes in [PNG, WEBP, GIF] {
        let image = ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .unwrap()
            .decode()
            .unwrap();
        assert_eq!((image.width(), image.height()), (2, 2));
        assert_eq!(image.to_rgb8().as_raw(), &expected);
    }
}
#[test]
fn decoded_allocation_and_dimension_limits_reject_before_pixels_are_returned() {
    for bytes in [PNG, WEBP, GIF] {
        let mut reader = ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .unwrap();
        let mut limits = Limits::default();
        limits.max_image_width = Some(1);
        limits.max_image_height = Some(1);
        reader.limits(limits);
        assert!(matches!(reader.decode(), Err(ImageError::Limits(_))));
    }
}
#[test]
fn truncated_inputs_and_bad_png_crc_remain_errors() {
    for bytes in [PNG, WEBP, GIF] {
        for n in [0, 1, 8, bytes.len() / 2] {
            assert!(
                ImageReader::new(Cursor::new(&bytes[..n]))
                    .with_guessed_format()
                    .unwrap()
                    .decode()
                    .is_err()
            );
        }
    }
    let mut bytes = PNG.to_vec();
    bytes[29] ^= 1;
    assert!(
        ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .unwrap()
            .decode()
            .is_err()
    );
}
#[test]
fn png_encode_keeps_exact_rgba_pixels() {
    let image = RgbaImage::from_raw(2, 1, vec![1, 2, 3, 4, 250, 249, 248, 247]).unwrap();
    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image.clone())
        .write_to(&mut out, ImageFormat::Png)
        .unwrap();
    assert_eq!(
        ImageReader::new(Cursor::new(out.into_inner()))
            .with_guessed_format()
            .unwrap()
            .decode()
            .unwrap()
            .to_rgba8(),
        image
    );
}

#[test]
fn resize_crop_and_orientation_preserve_the_existing_pixel_operations() {
    let image = ImageReader::new(Cursor::new(PNG))
        .with_guessed_format()
        .unwrap()
        .decode()
        .unwrap();
    assert_eq!(image.crop_imm(1, 0, 1, 1).to_rgb8().as_raw(), &[0, 255, 0]);
    let mut rotated = image;
    rotated.apply_orientation(metadata::Orientation::Rotate90);
    assert_eq!(
        rotated.to_rgb8().as_raw(),
        &[0, 0, 255, 255, 0, 0, 255, 255, 255, 0, 255, 0]
    );
    let constant = DynamicImage::ImageRgb8(RgbImage::from_pixel(3, 2, Rgb([12, 25, 253])));
    for filter in [
        imageops::FilterType::Triangle,
        imageops::FilterType::CatmullRom,
    ] {
        let scaled = constant.resize_exact(7, 5, filter).to_rgb8();
        assert_eq!(scaled.dimensions(), (7, 5));
        assert!(scaled.pixels().all(|p| p.0 == [12, 25, 253]));
    }
}
