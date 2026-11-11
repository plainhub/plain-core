//! Unit tests for `src/media/thumb_engine/pjpeg.rs` — moved out-of-line; compiled
//! as the `tests` child module via `#[cfg(test)] #[path]` there.
use super::*;

fn fixture(name: &str) -> Vec<u8> {
    let p = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../plain-rs/testdata"))
        .join(name);
    std::fs::read(&p).unwrap_or_else(|_| panic!("missing fixture {}", p.display()))
}

/// Reference: full JPEG decoding at one-eighth scale. scale.
/// The dev-only feature avoids CPU-specific SIMD rounding without weakening parity.
fn reference_1_8(data: &[u8]) -> Bitmap {
    let mut dec = crate::jpeg::Decoder::new(std::io::Cursor::new(data));
    dec.read_info().unwrap();
    let info = dec.info().unwrap();
    let w = u32::from(info.width);
    let h = u32::from(info.height);
    dec.scale((w / 8).max(1) as u16, (h / 8).max(1) as u16)
        .unwrap();
    let pixels = dec.decode().expect("reference decode");
    let info = dec.info().unwrap();
    if info.pixel_format == crate::jpeg::PixelFormat::L8 {
        let mut rgb = Vec::with_capacity(pixels.len() * 3);
        for v in &pixels {
            rgb.extend_from_slice(&[*v, *v, *v]);
        }
        return Bitmap::new_rgb(rgb, u32::from(info.width), u32::from(info.height));
    }
    Bitmap::new_rgb(pixels, u32::from(info.width), u32::from(info.height))
}

fn assert_same(a: &Bitmap, b: &Bitmap, ctx: &str) {
    assert_eq!(
        (a.width(), a.height()),
        (b.width(), b.height()),
        "{ctx} dims"
    );
    let (da, _, _) = a.raw();
    let (db, _, _) = b.raw();
    // The kernels mirror jpeg-decoder's scalar integer math exactly; measured
    // byte-identical on all fixtures. Allow nothing.
    assert!(
        da == db,
        "{ctx}: {} of {} bytes differ",
        da.iter().zip(db).filter(|(x, y)| x != y).count(),
        da.len()
    );
}

#[test]
fn parity_420_fixture() {
    let data = fixture("v-prog420.jpg");
    let mine = decode_dc_only_bytes(&data).unwrap();
    let r = reference_1_8(&data);
    assert_same(&mine, &r, "420 fixture");
    // 160x120 -> 20x15.
    assert_eq!((mine.width(), mine.height()), (20, 15));
}

#[test]
fn parity_grayscale_fixture() {
    let data = fixture("v-prog-gray.jpg");
    let mine = decode_dc_only_bytes(&data).unwrap();
    let r = reference_1_8(&data);
    assert_same(&mine, &r, "gray fixture");
    assert_eq!((mine.width(), mine.height()), (20, 15));
}

#[test]
fn parity_synthetic_fixture_and_skip_stats() {
    let data = fixture("v-synthetic-prog.jpg");
    let mine = decode_dc_only_bytes(&data).unwrap();
    assert_eq!((mine.width(), mine.height()), (150, 100));
    let r = reference_1_8(&data);
    assert_same(&mine, &r, "synthetic fixture");

    // The AC scans must be accounted as skipped: most of the entropy
    // payload never gets parsed.
    let (dc_bytes, skipped_bytes) = LAST_SKIP.get();
    assert!(
        skipped_bytes > dc_bytes * 2,
        "expected AC skip to dominate: dc={dc_bytes} skipped={skipped_bytes}"
    );
    println!("synthetic fixture: dc {dc_bytes} B decoded, {skipped_bytes} B AC skipped");
}

#[test]
fn dc_refinement_scan_is_decoded() {
    // The vips fixture's scan script has a trailing DC refinement scan
    // (Ah=1) after AC scans — it must not be skipped.
    let data = fixture("v-prog420.jpg");
    let mine = decode_dc_only_bytes(&data).unwrap();
    let r = reference_1_8(&data);
    assert_same(&mine, &r, "refinement scan parity");
}

#[test]
fn sequential_dc_matches_scalar_decoder_with_nonzero_ac_coefficients() {
    for (w, h) in [(64, 48), (127, 93), (319, 257)] {
        let image = crate::image::RgbImage::from_fn(w, h, |x, y| {
            crate::image::Rgb([
                (x * 13 + y * 7) as u8,
                (x * 3 + y * 19) as u8,
                (x * 23 + y * 11) as u8,
            ])
        });
        let mut bytes = Vec::new();
        crate::image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 85)
            .encode(image.as_raw(), w, h, crate::image::ExtendedColorType::Rgb8)
            .unwrap();
        let mut decoder = Decoder::new(&bytes).unwrap();
        decoder.six_bit_color = false;
        let decoded = decoder.decode(false, None).unwrap();
        assert_same(&decoded, &reference_1_8(&bytes), "sequential AC skipping");
        let mut decoder = Decoder::new(&bytes).unwrap();
        decoder.six_bit_color = true;
        let full = decoder.decode(false, None).unwrap();
        for crop in [(1, 1), (7, 3), (3, 19)] {
            let mut decoder = Decoder::new(&bytes).unwrap();
            decoder.six_bit_color = true;
            let cropped = decoder.decode(false, Some(crop)).unwrap();
            let (raw, w, h) = full.raw();
            let expected =
                super::super::scale::crop_for_fill(Bitmap::new_rgb(raw.to_vec(), w, h), Some(crop));
            assert_same(&cropped, &expected, "vector color crop phase");
        }
    }
}

#[test]
fn vertical_chroma_filter_preserves_phase_and_edges() {
    let frame = Frame {
        width: 16,
        height: 32,
        comps: vec![
            ComponentSpec {
                id: 1,
                sx: 1,
                sy: 2,
                quant_id: 0,
            },
            ComponentSpec {
                id: 2,
                sx: 1,
                sy: 1,
                quant_id: 0,
            },
        ],
        hmax: 1,
        vmax: 2,
    };
    let plane = [10, 30, 90, 170];
    for (row, expected) in [[10, 30], [30, 65], [70, 135], [90, 170]]
        .into_iter()
        .enumerate()
    {
        let mut out = [0; 2];
        upsample_row(&plane, 2, 2, row, &frame.comps[1], &frame, &mut out);
        assert_eq!(out, expected);
    }
}

#[test]
fn rejects_malformed_streams() {
    let data = fixture("v-prog420.jpg");
    // Truncated inside the DC scan: cut early.
    let cut = &data[..data.len() / 3];
    // May or may not error depending on where the cut lands relative to
    // the DC scan, but must never panic and must produce a bitmap or Err.
    let _ = decode_dc_only_bytes(cut);

    // Garbage payload after valid headers of the gray fixture.
    let mut bad = fixture("v-prog-gray.jpg");
    let half = bad.len() / 2;
    for b in bad.iter_mut().skip(half) {
        *b = 0x5A;
    }
    let _ = decode_dc_only_bytes(&bad);

    // Not a JPEG at all.
    assert!(decode_dc_only_bytes(b"hello world, definitely not jpeg").is_err());
}

#[test]
fn h2v1_and_h2v2_row_math() {
    // Pin the fir kernels to jpeg-decoder's exact integer math.
    // H2V1: input [100, 200], out = [100, (300+200+2)/4=125? no: (3*100+200+2)>>2=125]
    let mut out = [0u8; 4];
    h2v1_row(&[100, 200], 2, 1, 0, &mut out);
    assert_eq!(
        out,
        [
            100,
            ((3 * 100 + 200 + 2) >> 2) as u8,
            ((3 * 200 + 100 + 2) >> 2) as u8,
            200
        ]
    );

    // H2V2 single column: value = (3*near + far + 2) >> 2.
    let plane = [10, 90]; // two rows, one column
    let mut o = [0u8; 2];
    h2v2_row(&plane, 1, 2, 0, &mut o); // even row: near=0, far=0 (clamped)
    assert_eq!(o, [((3 * 10 + 10 + 2) >> 2) as u8; 2]);
    h2v2_row(&plane, 1, 2, 1, &mut o); // odd row: near=0, far=1
    assert_eq!(o, [((3 * 10 + 90 + 2) >> 2) as u8; 2]);
    h2v2_row(&plane, 1, 2, 2, &mut o); // near=1, far=0
    assert_eq!(o, [((3 * 90 + 10 + 2) >> 2) as u8; 2]);
    h2v2_row(&plane, 1, 2, 3, &mut o); // near=1, far=1 (clamped)
    assert_eq!(o, [((3 * 90 + 90 + 2) >> 2) as u8; 2]);
}

#[test]
fn ycbcr_matches_jpeg_decoder_constants() {
    // Spot-check against the float reference: y=128, cb=128, cr=128 is neutral.
    assert_eq!(ycbcr_to_rgb(128, 128, 128), (128, 128, 128));
    let (r, g, b) = ycbcr_to_rgb(200, 100, 180);
    assert!(r > 200 && g < 200 && b > 100, "r={r} g={g} b={b}");
}

#[test]
fn extend_categories() {
    assert_eq!(extend(0b0, 1), -1);
    assert_eq!(extend(0b1, 1), 1);
    assert_eq!(extend(0b00, 2), -3);
    assert_eq!(extend(0b11, 2), 3);
}

#[test]
fn fast_dc_prefix_consumes_only_the_code_and_magnitude() {
    let mut counts = [0; 16];
    counts[1] = 3;
    counts[8] = 1;
    let t = HuffTable::build(&counts, &[0, 3, 10, 2]).unwrap();
    for suffix in 0..8 {
        let data = [0b0100_0000 | suffix << 3 | 5];
        let mut bits = Bits::new(&data, 0);
        assert_eq!(
            t.difference(&mut bits).unwrap(),
            extend(i32::from(suffix), 3)
        );
        assert_eq!(bits.receive(3), Some(5));
    }
    // Two-bit code 10 plus a ten-bit magnitude; nine-bit code 110000000 plus two magnitude bits.
    let data = [0b1010_1010, 0b1010_1100, 0b0000_0101];
    let mut bits = Bits::new(&data, 0);
    assert_eq!(t.difference(&mut bits).unwrap(), 682);
    assert_eq!(t.difference(&mut bits).unwrap(), 2);
    assert_eq!(bits.receive(1), Some(1));
}

#[test]
fn dc_bit_reader_unstuffs_bytes_and_rejects_a_truncated_magnitude() {
    let bytes = [0xab, 0xff, 0, 0xcd, 0xff, 0xd0];
    let mut bits = Bits::new(&bytes, 0);
    assert_eq!(bits.receive(4), Some(0xa));
    assert_eq!(bits.receive(16), Some(0xbffc));
    assert_eq!(bits.receive(4), Some(0xd));
    assert_eq!(bits.receive(1), None);
    assert!(bits.consume_restart());
    let mut counts = [0; 16];
    counts[0] = 1;
    let table = HuffTable::build(&counts, &[7]).unwrap();
    let mut bits = Bits::new(&[0, 0xff, 0xd9], 0);
    assert_eq!(bits.receive(1), Some(0));
    assert!(table.difference(&mut bits).is_err());
}

#[test]
fn skipped_ac_scan_stops_at_the_first_marker_at_every_word_offset() {
    for pad in 0..24 {
        for restart in [0x00, 0xd0, 0xd7] {
            let mut bytes = vec![0xff, 0xd8];
            bytes.resize(2 + pad, 0x11);
            bytes.extend_from_slice(&[0xff, restart]);
            bytes.extend_from_slice(&[0x44; 16]);
            let end = bytes.len();
            bytes.extend_from_slice(&[0xff, 0xda, 0, 8]);
            let mut decoder = Decoder::new(&bytes).unwrap();
            decoder.skip_scan_payload();
            assert_eq!(decoder.pos, end);
        }
    }
}

#[test]
fn complete_dc_header_serves_the_same_pixels_without_opening_the_source() {
    let mut data = fixture("v-prog420.jpg");
    data.splice(
        2..2,
        [
            0xff, 0xe0, 0, 16, b'J', b'F', b'I', b'F', 0, 1, 1, 0, 0, 1, 0, 1, 0, 0,
        ],
    );
    let sos = data.windows(2).position(|b| b == [0xff, 0xda]).unwrap();
    let start = sos + 2 + usize::from(u16::from_be_bytes([data[sos + 2], data[sos + 3]]));
    assert!(Decoder::new(&data).unwrap().decode(true, None).is_err());
    data[start - 1] = 0;
    let mut scan = Decoder::new(&data).unwrap();
    scan.pos = start;
    scan.skip_scan_payload();
    let end = scan.pos;
    data.truncate(end);
    data.extend_from_slice(&[0xff, 0xd9]);
    let expected = reference_1_8(&data);
    assert!(
        Decoder::new(&data[..end - 1])
            .unwrap()
            .decode(true, None)
            .is_err()
    );
    let header = super::super::sniff::sniff_header(&data).unwrap();
    let decoded =
        super::super::decode_to_target("absent-source.jpg", &data, header, 8, 8, None).unwrap();
    assert_same(&decoded, &expected, "DC header");
    data.drain(2..20);
    assert!(Decoder::new(&data).unwrap().decode(true, None).is_err());
    assert_same(
        &decode_dc_only_bytes(&data).unwrap(),
        &expected,
        "undeclared color",
    );
}

#[test]
fn prefix_lookahead_rewinds_exact_encoded_bytes_including_stuffing() {
    let data = [0x12, 0x34, 0x56, 0x78, 0xff, 0, 0xab, 0xff, 0xd4];
    let mut bits = Bits::new(&data, 0);
    assert_eq!(bits.peek_prefix(), 0x48);
    assert_eq!(bits.pos_after(), 0);
    for value in [0x12, 0x34, 0x56, 0x78] {
        assert_eq!(bits.receive(8), Some(value));
    }
    assert_eq!(bits.peek_prefix(), 0x3fe);
    assert_eq!(bits.pos_after(), 4);
    bits.align_to_byte();
    assert_eq!(bits.receive(16), Some(0xffab));
    assert!(bits.consume_restart());
}

#[test]
fn invalid_quantization_ids_return_errors() {
    let data = fixture("v-prog420.jpg");
    for (marker, field_offset) in [(0xdb, 4), (0xc2, 12)] {
        let mut broken = data.clone();
        let i = broken
            .windows(2)
            .position(|bytes| bytes == [0xff, marker])
            .unwrap();
        broken[i + field_offset] = 4;
        assert!(decode_dc_only_bytes(&broken).is_err());
    }
}

fn reference_difference(table: &HuffTable, bits: &mut Bits) -> Option<i32> {
    let category = u32::from(table.codes.symbol(bits).ok()?);
    if category == 0 {
        return Some(0);
    }
    let magnitude = bits.receive(category)? as i32;
    Some(extend(magnitude, category))
}

#[test]
fn grouped_dc_codes_match_serial_decoding_at_every_bit_offset() {
    let mut counts = [0; 16];
    counts[0] = 1;
    let y = HuffTable::build(&counts, &[3]).unwrap();
    let cb = HuffTable::build(&counts, &[0]).unwrap();
    let cr = HuffTable::build(&counts, &[11]).unwrap();
    let bytes = [
        0x12, 0x34, 0x08, 0x04, 0x24, 0x06, 0x14, 0x68, 0x12, 0x40, 0x24, 0x40, 0, 0, 0, 0,
    ];
    for offset in 0..8 {
        let mut data = vec![0; 2];
        let mut output_bit = offset;
        // Emit four signed 3-bit luma deltas, zero Cb, and a partially tabled 11-bit Cr.
        for (magnitude, width) in [(4u16, 3), (2, 3), (5, 3), (1, 3), (0, 0), (0x642, 11)] {
            for bit in (0..=width).rev() {
                if output_bit / 8 >= data.len() {
                    data.push(0)
                }
                data[output_bit / 8] |= (((magnitude >> bit) & 1) as u8) << (7 - output_bit % 8);
                output_bit += 1;
            }
        }
        data.extend_from_slice(&bytes);
        let mut grouped = Bits::new(&data, 0);
        let mut serial = Bits::new(&data, 0);
        for b in [&mut grouped, &mut serial] {
            b.receive(offset as u32).unwrap();
        }
        let expected =
            [&y, &y, &y, &y, &cb, &cr].map(|t| reference_difference(t, &mut serial).unwrap());
        assert_eq!(expected, [4, -5, 5, -6, 0, 0x642]);
        assert_eq!(grouped.try_420_window([&y, &cb, &cr]), Some(expected));
        assert_eq!(grouped.pos_after(), serial.pos_after());
        assert_eq!(grouped.receive(16), serial.receive(16));
    }
}

#[test]
fn grouped_dc_falls_back_without_consuming_long_codes_stuffing_or_markers() {
    let mut counts = [0; 16];
    counts[0] = 1;
    counts[10] = 1;
    let table = HuffTable::build(&counts, &[0, 10]).unwrap();
    for data in [
        &[0x80, 0, 0x40, 0, 0, 0, 0, 0, 0, 0][..],
        &[0, 0xff, 0, 4, 5, 6, 7, 8, 9],
        &[0, 0xff, 0xd0],
        &[0x80, 0xff, 0xd9],
    ] {
        let mut grouped = Bits::new(data, 0);
        assert!(grouped.try_420_window([&table; 3]).is_none());
        assert_eq!((grouped.pos, grouped.cur, grouped.cnt), (0, 0, 0));
        let mut serial = Bits::new(data, 0);
        let expected = (0..6)
            .map(|_| reference_difference(&table, &mut serial))
            .collect::<Option<Vec<_>>>();
        assert_eq!(
            grouped.differences_420([&table; 3]).ok().map(Vec::from),
            expected
        );
    }
    let mut short = [0; 16];
    short[0] = 1;
    let wide = HuffTable::build(&short, &[11]).unwrap();
    let data = [0; 16];
    let mut bits = Bits::new(&data, 0);
    assert!(bits.try_420_window([&wide; 3]).is_none());
    assert_eq!(bits.pos_after(), 0);
    assert_eq!(bits.differences_420([&wide; 3]).unwrap(), [-2047; 6]);
    assert_eq!(bits.pos_after(), 9);
}

#[test]
fn cropped_dc_reconstruction_matches_the_full_grid_with_chroma_neighbors() {
    for file in ["v-synthetic-prog.jpg", "v-prog420.jpg", "v-prog-gray.jpg"] {
        let data = fixture(file);
        for (w, h) in [(1, 1), (3, 7), (9, 4), (1, 19), (20, 1)] {
            let full = reference_1_8(&data);
            let expected = super::super::scale::crop_for_fill(full, Some((w, h)));
            PIXELS_RECONSTRUCTED.set(0);
            let cropped = Decoder::new(&data)
                .unwrap()
                .decode(false, Some((w, h)))
                .unwrap();
            assert_same(&cropped, &expected, file);
            assert_eq!(
                PIXELS_RECONSTRUCTED.get(),
                (expected.width() * expected.height()) as usize
            );
        }
    }
}
