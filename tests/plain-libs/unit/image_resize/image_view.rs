use super::*;
use crate::image_resize::{images::TypedImage, pixels::U8x3};

#[test]
fn odd_vertical_stripes_are_disjoint_and_preserve_outside_columns() {
    let mut pixels = vec![U8x3::new([0; 3]); 35];
    let mut image = TypedImage::from_pixels_slice(7, 5, &mut pixels).unwrap();
    {
        let mut parts = image
            .split_by_width_mut(1, NonZeroU32::new(5).unwrap(), NonZeroU32::new(3).unwrap())
            .unwrap();
        assert_eq!(
            parts.iter().map(ImageView::width).collect::<Vec<_>>(),
            [2, 2, 1]
        );
        for (index, part) in parts.iter_mut().enumerate() {
            for row in part.iter_rows_mut(0) {
                for pixel in row {
                    pixel.0 = [index as u8 + 1; 3];
                }
            }
        }
    }
    for row in image.iter_rows(0) {
        assert_eq!(
            row.iter().map(|pixel| pixel.0[0]).collect::<Vec<_>>(),
            [0, 1, 1, 2, 2, 3, 0]
        );
    }
}

#[test]
fn nested_row_partitions_preserve_remainders_and_empty_slices() {
    let mut pixels = vec![U8x3::new([0; 3]); 21];
    let mut image = TypedImage::from_pixels_slice(3, 7, &mut pixels).unwrap();
    {
        let mut stripes = image
            .split_by_width_mut(0, NonZeroU32::new(3).unwrap(), NonZeroU32::new(2).unwrap())
            .unwrap();
        let mut rows = stripes[0]
            .split_by_height_mut(1, NonZeroU32::new(5).unwrap(), NonZeroU32::new(2).unwrap())
            .unwrap();
        assert_eq!(
            rows.iter().map(ImageView::height).collect::<Vec<_>>(),
            [3, 2]
        );
        for (index, part) in rows.iter_mut().enumerate() {
            assert_eq!(part.iter_rows_mut(part.height()).count(), 0);
            for row in part.iter_rows_mut(0) {
                for pixel in row {
                    pixel.0 = [index as u8 + 1; 3];
                }
            }
        }
    }
    assert_eq!(
        image
            .iter_rows(0)
            .flatten()
            .map(|pixel| pixel.0[0])
            .collect::<Vec<_>>(),
        [
            0, 0, 0, 1, 1, 0, 1, 1, 0, 1, 1, 0, 2, 2, 0, 2, 2, 0, 0, 0, 0
        ]
    );
    assert!(
        image
            .split_by_width_mut(2, NonZeroU32::new(2).unwrap(), NonZeroU32::new(1).unwrap())
            .is_none()
    );
}

#[test]
fn image_dimensions_cannot_wrap_into_an_undersized_byte_buffer() {
    use crate::image_resize::{ImageBufferError, PixelType, images::Image};
    assert!(matches!(
        Image::from_vec_u8(1 << 31, 1 << 31, Vec::new(), PixelType::U8x4),
        Err(ImageBufferError::InvalidBufferSize)
    ));
    assert!(Image::from_vec_u8(2, 2, vec![0; 15], PixelType::U8x4).is_err());
    assert!(Image::from_vec_u8(2, 2, vec![0; 16], PixelType::U8x4).is_ok());
}

#[test]
fn typed_views_expose_only_the_declared_image_not_trailing_pixels() {
    use crate::image_resize::images::TypedImageRef;
    let pixels = [U8x3::new([1; 3]), U8x3::new([2; 3])];
    let image = TypedImageRef::new(1, 1, &pixels).unwrap();
    assert_eq!(image.iter_rows(0).count(), 1);
    let mut pixels = pixels.to_vec();
    let mut image = TypedImage::from_pixels_slice(1, 1, &mut pixels).unwrap();
    assert_eq!(image.iter_rows_mut(0).count(), 1);
    assert_eq!(image.iter_rows(0).flatten().count(), 1);
}
