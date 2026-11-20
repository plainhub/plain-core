use super::*;
use crate::image_resize::images::Image;
use crate::media::thumbnails::scale::{Bitmap, resize_to};
use std::cell::Cell;

thread_local! {
    pub(super) static LARGEST_TEMP: Cell<usize> = const { Cell::new(0) };
}

#[test]
fn owned_rgba_thumbnail_matches_borrowed_resize_without_full_source_scratch() {
    let (w, h) = (339, 269);
    let bytes: Vec<u8> = (0..w * h)
        .flat_map(|i| [(i * 7) as u8, (i / 13) as u8, (i / 3) as u8, i as u8])
        .collect();
    for (tw, th, filter) in [
        (256, 210, FilterType::Lanczos3),
        (191, 173, FilterType::Bilinear),
    ] {
        let src = Image::from_vec_u8(w, h, bytes.clone(), PixelType::U8x4).unwrap();
        let mut reference = Image::new(tw, th, PixelType::U8x4);
        Resizer::new().resize(&src, &mut reference, filter).unwrap();
        LARGEST_TEMP.set(0);
        let result = resize_to(Bitmap::new_rgba(bytes.clone(), w, h), tw, th);
        assert_eq!(result.raw().0, reference.buffer());
        assert!(
            LARGEST_TEMP.get() < bytes.len(),
            "scratch {} >= source {}",
            LARGEST_TEMP.get(),
            bytes.len()
        );
    }
}
