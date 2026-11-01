use super::dimensions;
use super::exif_gps;

#[test]
fn ico_dimensions_parse() {
    // ICONDIR: reserved(0,0) | type(1) | count(2) | first entry w/h.
    let mut b = Vec::new();
    b.extend_from_slice(&[0x00, 0x00, 0x01, 0x00, 0x03, 0x00]); // header, 3 images
    b.extend_from_slice(&[0x10, 0x10]); // first entry: w=16, h=16
    assert_eq!(dimensions(&b), Some((16, 16)));
}

#[test]
fn ico_zero_width_means_256() {
    let mut b = Vec::new();
    b.extend_from_slice(&[0x00, 0x00, 0x01, 0x00, 0x01, 0x00]);
    b.extend_from_slice(&[0x00, 0x00]); // 0 => 256
    assert_eq!(dimensions(&b), Some((256, 256)));
}

#[test]
fn png_dimensions_parse() {
    let mut b = Vec::new();
    b.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    b.extend_from_slice(&[0x00, 0x00, 0x00, 0x0D]); // IHDR length
    b.extend_from_slice(b"IHDR");
    b.extend_from_slice(&[0x00, 0x00, 0x01, 0x00]); // width = 256
    b.extend_from_slice(&[0x00, 0x00, 0x00, 0x40]); // height = 64
    assert_eq!(dimensions(&b), Some((256, 64)));
}

#[test]
fn jpeg_dimensions_parse() {
    // FF D8 | FF C0 | seg(2) | precis(1) | h(2) | w(2)
    let b = [
        0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0x20, 0x01, 0x00,
    ];
    assert_eq!(dimensions(&b), Some((256, 32)));
}

#[test]
fn gif_dimensions_parse() {
    let mut b = Vec::new();
    b.extend_from_slice(b"GIF89a");
    b.extend_from_slice(&[0xFF, 0x00, 0x40, 0x00]); // w=255, h=64 (LE)
    assert_eq!(dimensions(&b), Some((255, 64)));
}

#[test]
fn bmp_dimensions_parse() {
    let mut b = Vec::new();
    b.extend_from_slice(b"BM");
    b.resize(18, 0);
    b.extend_from_slice(&[0x00, 0x02, 0x00, 0x00]); // width = 512 (LE)
    b.extend_from_slice(&[0x64, 0x00, 0x00, 0x00]); // height = 100 (LE)
    assert_eq!(dimensions(&b), Some((512, 100)));
}

#[test]
fn webp_vp8l_dimensions_parse() {
    // RIFF | size | WEBP | VP8L | signature | packed dims (zeros => 1x1)
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&[0, 0, 0, 0]);
    b.extend_from_slice(b"WEBP");
    b.extend_from_slice(b"VP8L");
    b.extend_from_slice(&[0x2F, 0x00, 0x00, 0x00, 0x00]);
    assert_eq!(dimensions(&b), Some((1, 1)));
}

#[test]
fn unknown_format_is_none() {
    assert_eq!(dimensions(b"not an image at all"), None);
    assert_eq!(dimensions(&[]), None);
}

// ── TIFF / EXIF helpers ──────────────────────────────────────────────

/// Build a minimal little-endian TIFF: header + one IFD with `entries`
/// (tag, type, count, inline value) and room for extra data at `extra_off`.
fn build_le_tiff(
    entries: &[(u16, u16, u32, u32)],
    extra: &[(usize, Vec<u8>)],
    total: usize,
) -> Vec<u8> {
    let mut b = vec![0u8; total];
    b[0..2].copy_from_slice(b"II");
    b[2..4].copy_from_slice(&0x2Au16.to_le_bytes());
    b[4..8].copy_from_slice(&8u32.to_le_bytes()); // IFD0 at 8
    b[8..10].copy_from_slice(&(entries.len() as u16).to_le_bytes());
    for (i, (tag, typ, count, value)) in entries.iter().enumerate() {
        let e = 10 + i * 12;
        b[e..e + 2].copy_from_slice(&tag.to_le_bytes());
        b[e + 2..e + 4].copy_from_slice(&typ.to_le_bytes());
        b[e + 4..e + 8].copy_from_slice(&count.to_le_bytes());
        b[e + 8..e + 12].copy_from_slice(&value.to_le_bytes());
    }
    let next = 10 + entries.len() * 12;
    b[next..next + 4].copy_from_slice(&0u32.to_le_bytes());
    for (off, data) in extra {
        b[*off..*off + data.len()].copy_from_slice(data);
    }
    b
}

#[test]
fn tiff_dimensions_parse() {
    // IFD0 at 8: width (0x0100, SHORT=3) = 256, height (0x0101, SHORT=3) = 64.
    let b = build_le_tiff(
        &[(0x0100, 3, 1, 256), (0x0101, 3, 1, 64)],
        &[],
        10 + 2 * 12 + 4,
    );
    assert_eq!(dimensions(&b), Some((256, 64)));
}

#[test]
fn tiff_big_endian_dimensions_parse() {
    let mut b = vec![0u8; 58];
    b[0..2].copy_from_slice(b"MM");
    b[2..4].copy_from_slice(&42u16.to_be_bytes());
    b[4..8].copy_from_slice(&8u32.to_be_bytes());
    b[8..10].copy_from_slice(&2u16.to_be_bytes());
    // width LONG (type 4) = 512, height LONG = 64, big-endian entries.
    b[10..12].copy_from_slice(&0x0100u16.to_be_bytes());
    b[12..14].copy_from_slice(&4u16.to_be_bytes());
    b[14..18].copy_from_slice(&1u32.to_be_bytes());
    b[18..22].copy_from_slice(&512u32.to_be_bytes());
    b[22..24].copy_from_slice(&0x0101u16.to_be_bytes());
    b[24..26].copy_from_slice(&4u16.to_be_bytes());
    b[26..30].copy_from_slice(&1u32.to_be_bytes());
    b[30..34].copy_from_slice(&64u32.to_be_bytes());
    b[34..38].copy_from_slice(&0u32.to_be_bytes());
    assert_eq!(dimensions(&b), Some((512, 64)));
}

#[test]
fn exif_gps_from_jpeg_roundtrip() {
    // GPS IFD at offset 46: N, 37°0'0", W, 122°0'0".
    let mut gps_ifd = Vec::new();
    gps_ifd.extend_from_slice(&4u16.to_le_bytes()); // 4 entries
    let lat_off: usize = 46 + 2 + 4 * 12 + 4;
    let lon_off: usize = lat_off + 24; // 3 RATIONals = 24 bytes
    let e = |tag: u16, typ: u16, count: u32, value: u32, buf: &mut Vec<u8>| {
        buf.extend_from_slice(&tag.to_le_bytes());
        buf.extend_from_slice(&typ.to_le_bytes());
        buf.extend_from_slice(&count.to_le_bytes());
        buf.extend_from_slice(&value.to_le_bytes());
    };
    // GPSLatitudeRef "N\0" (ASCII type 2, count 2, inline)
    let n_ref = [b'N', 0, 0, 0];
    e(0x0001, 2, 2, u32::from_le_bytes(n_ref), &mut gps_ifd);
    // GPSLatitude RATIONAL×3 at lat_off
    e(0x0002, 5, 3, lat_off as u32, &mut gps_ifd);
    // GPSLongitudeRef "W\0"
    let w_ref = [b'W', 0, 0, 0];
    e(0x0003, 2, 2, u32::from_le_bytes(w_ref), &mut gps_ifd);
    // GPSLongitude RATIONAL×3 at lon_off
    e(0x0004, 5, 3, lon_off as u32, &mut gps_ifd);
    gps_ifd.extend_from_slice(&0u32.to_le_bytes()); // next IFD

    let mut rationals = Vec::new();
    for (n, d) in [(37u32, 1u32), (0, 1), (0, 1)] {
        rationals.extend_from_slice(&n.to_le_bytes());
        rationals.extend_from_slice(&d.to_le_bytes());
    }
    let mut lon_rationals = Vec::new();
    for (n, d) in [(122u32, 1u32), (0, 1), (0, 1)] {
        lon_rationals.extend_from_slice(&n.to_le_bytes());
        lon_rationals.extend_from_slice(&d.to_le_bytes());
    }

    // IFD0 with a single GPS-info pointer (0x8825, LONG) → 46.
    let tiff = build_le_tiff(
        &[(0x8825, 4, 1, 46)],
        &[
            (46, gps_ifd),
            (lat_off, rationals),
            (lon_off, lon_rationals),
        ],
        256,
    );

    // Wrap in a JPEG APP1 segment: SOI + APP1(len) + "Exif\0\0" + tiff + EOI.
    let mut jpeg = Vec::new();
    jpeg.extend_from_slice(&[0xFF, 0xD8]);
    jpeg.extend_from_slice(&[0xFF, 0xE1]);
    let seg_len = (2 + 6 + tiff.len()) as u16;
    jpeg.extend_from_slice(&seg_len.to_be_bytes());
    jpeg.extend_from_slice(b"Exif\0\0");
    jpeg.extend_from_slice(&tiff);
    jpeg.extend_from_slice(&[0xFF, 0xD9]);

    assert_eq!(exif_gps(&jpeg), Some((37.0, -122.0)));
    // A bare TIFF payload works too.
    assert_eq!(exif_gps(&tiff), Some((37.0, -122.0)));
}

#[test]
fn exif_gps_absent_is_none() {
    assert_eq!(exif_gps(b"nope"), None);
    // TIFF without GPS IFD.
    let b = build_le_tiff(&[(0x0100, 3, 1, 5)], &[], 40);
    assert_eq!(exif_gps(&b), None);
}
