use std::cell::Cell;

thread_local! {
    pub(super) static ROW_BUFFER_SETS: Cell<usize> = const { Cell::new(0) };
}

#[test]
fn decoding_color_jpeg_allocates_one_set_of_row_buffers() {
    let image = crate::image::RgbImage::from_fn(128, 96, |x, y| {
        crate::image::Rgb([x as u8, y as u8, (x + y) as u8])
    });
    let mut bytes = Vec::new();
    crate::image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 85)
        .encode(image.as_raw(), 128, 96, crate::image::ExtendedColorType::Rgb8)
        .unwrap();
    ROW_BUFFER_SETS.set(0);
    let mut decoder = crate::jpeg::Decoder::new(bytes.as_slice());
    decoder.scale(64, 48).unwrap();
    assert_eq!(decoder.decode().unwrap().len(), 64 * 48 * 3);
    assert_eq!(ROW_BUFFER_SETS.get(), 1);
}
