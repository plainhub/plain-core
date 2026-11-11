use super::super::tests::fixtures::{Counted, expanded, fixture};
use super::*;
use std::io::Cursor;

#[test]
fn streaming_box_decode_matches_full_crop_and_box_pixels_in_both_row_orders() {
    for name in ["rgb24.bmp", "rgb32.bmp", "rgb88832.bmp", "alpha32.bmp"] {
        for top_down in [false, true] {
            for crop in [None, Some((11, 7)), Some((7, 11))] {
                let bytes = expanded(name, 129, 67, top_down);
                let full = Bitmap::from_dynamic(
                    crate::image::DynamicImage::from_decoder(
                        BmpDecoder::new(Cursor::new(bytes.clone())).unwrap(),
                    )
                    .unwrap(),
                );
                let mut decoder = BmpDecoder::new(Counted {
                    input: Cursor::new(bytes),
                    reads: 0,
                    bytes: 0,
                    largest_read: 0,
                })
                .unwrap();
                let header_bytes = decoder.reader.bytes;
                let header_calls = decoder.reader.reads;
                let (cw, ch) = scale::crop_size(129, 67, crop);
                let factor = (cw / 11).min(ch / 7).min(64);
                let reduced = decoder
                    .read_scaled(11, 7, crop)
                    .unwrap()
                    .expect("streamable BMP");
                let expected = scale::box_downscale(&scale::crop_for_fill(full, crop), factor);
                assert_eq!(
                    reduced.raw(),
                    expected.raw(),
                    "{name} top={top_down} crop={crop:?}"
                );
                let source_channels = if name == "rgb24.bmp" { 3 } else { 4 };
                assert_eq!(
                    decoder.reader.bytes - header_bytes,
                    cw as usize * ch as usize * source_channels
                );
                assert_eq!(decoder.reader.reads - header_calls, ch as usize);
                assert!(decoder.reader.largest_read <= cw as usize * source_channels);
                assert!(reduced.raw().0.len() < 129 * 67 * source_channels / 4);
            }
        }
    }
}

#[test]
fn streaming_is_selected_only_when_the_layout_and_reduction_are_supported() {
    for name in ["rle4.bmp", "rle8.bmp", "palette8.bmp", "rgb565.bmp"] {
        let mut decoder = BmpDecoder::new(Cursor::new(fixture(name))).unwrap();
        assert!(decoder.read_scaled(1, 1, None).unwrap().is_none(), "{name}");
    }
    let mut decoder = BmpDecoder::new(Cursor::new(fixture("rgb24.bmp"))).unwrap();
    assert!(decoder.read_scaled(3, 2, None).unwrap().is_none());
    let mut bytes = expanded("rgb24.bmp", 129, 67, true);
    bytes.pop();
    let mut decoder = BmpDecoder::new(Cursor::new(bytes)).unwrap();
    assert!(decoder.read_scaled(7, 11, Some((7, 11))).is_err());
}

#[test]
#[ignore]
fn batch_thumbnail_benchmark() {
    let directory = crate::temporary::tempdir().unwrap();
    let path = directory.path().join("batch.bmp");
    std::fs::write(&path, expanded("rgb24.bmp", 4096, 3072, false)).unwrap();
    let decode = |streaming: bool| {
        let bitmap = if streaming {
            crate::media::thumbnails::bmp::thumbnail(&path, 256, 256, Some((256, 256))).unwrap()
        } else {
            let input = std::io::BufReader::new(std::fs::File::open(&path).unwrap());
            let bitmap = Bitmap::from_dynamic(
                crate::image::DynamicImage::from_decoder(BmpDecoder::new(input).unwrap()).unwrap(),
            );
            scale::resize_to(scale::crop_for_fill(bitmap, Some((256, 256))), 256, 256)
        };
        crate::media::thumbnails::encode::encode_jpeg(bitmap, 85).unwrap()
    };
    let expected = decode(false);
    assert_eq!(decode(true), expected);
    let workers = std::thread::available_parallelism().unwrap().get().min(4);
    let run = |streaming: bool| {
        let start = std::time::Instant::now();
        std::thread::scope(|scope| {
            for _ in 0..workers {
                let decode = &decode;
                let expected = &expected;
                scope.spawn(move || {
                    for _ in 0..3 {
                        assert_eq!(decode(streaming), *expected);
                    }
                });
            }
        });
        start.elapsed().as_secs_f64()
    };
    let before = run(false);
    let after = run(true);
    eprintln!(
        "BMP batch os={} architecture={} input=4096x3072 output=256x256 workers={} images={} warmSourcePages=true thumbnailCache=false beforeSeconds={before:.6} afterSeconds={after:.6} outputBytes={} outputsIdentical=true",
        std::env::consts::OS,
        std::env::consts::ARCH,
        workers,
        workers * 3,
        expected.len()
    );
}
