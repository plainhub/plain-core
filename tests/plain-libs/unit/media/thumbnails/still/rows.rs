use super::super::super::scale::{Bitmap, box_downscale};
use super::*;

#[test]
fn row_filter_matches_rgb_and_rgba_rounding_with_bounded_scratch() {
    fn check<const C: usize>() {
        for (w, h, tw, th) in [
            (129, 67, 16, 16),
            (17, 13, 8, 8),
            (130, 131, 1, 1),
            (7, 5, 32, 32),
        ] {
            let data: Vec<u8> = (0..w * h * C as u32)
                .map(|n| ((n * 17 + 31) % 256) as u8)
                .collect();
            let mut rows = Rows::<C>::new(w, h, tw, th);
            assert_eq!(
                rows.vertical.len(),
                if rows.factor == 1 { 0 } else { w as usize * C }
            );
            let factor = rows.factor;
            for row in data.chunks_exact(w as usize * C) {
                rows.push(row);
                assert!(rows.buffered < factor);
            }
            let source = if C == 3 {
                Bitmap::new_rgb(data, w, h)
            } else {
                Bitmap::new_rgba(data, w, h)
            };
            let expected = if factor == 1 {
                source
            } else {
                box_downscale(&source, factor)
            };
            let (actual, aw, ah) = rows.finish();
            assert_eq!((actual.as_slice(), aw, ah), expected.raw());
        }
    }
    check::<3>();
    check::<4>();
}

#[test]
fn first_row_of_each_group_overwrites_the_previous_filtered_sums() {
    let mut rows = Rows::<3>::new(2, 5, 1, 2);
    let scratch = rows.vertical.as_ptr();
    rows.push(&[200; 6]);
    rows.push(&[100; 6]);
    assert_eq!(rows.vertical, [150; 6]);
    rows.push(&[7, 11, 13, 17, 19, 23]);
    assert_eq!(rows.vertical, [7, 11, 13, 17, 19, 23]);
    rows.push(&[9; 6]);
    rows.push(&[42; 6]);
    assert_eq!(rows.vertical.as_ptr(), scratch);
    let (pixels, width, height) = rows.finish();
    assert_eq!((width, height), (1, 3));
    assert_eq!(&pixels[pixels.len() - 3..], &[42; 3]);
}
