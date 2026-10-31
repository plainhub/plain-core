use super::*;
use std::io::Cursor;

fn u16_at(bytes: &mut [u8], offset: usize, value: u16, little: bool) {
    bytes[offset..offset + 2].copy_from_slice(&if little {
        value.to_le_bytes()
    } else {
        value.to_be_bytes()
    });
}
fn u32_at(bytes: &mut [u8], offset: usize, value: u32, little: bool) {
    bytes[offset..offset + 4].copy_from_slice(&encode_u32(value, little));
}
fn entry(
    bytes: &mut [u8],
    offset: usize,
    tag: u16,
    kind: u16,
    count: u32,
    value: u32,
    little: bool,
) {
    u16_at(bytes, offset, tag, little);
    u16_at(bytes, offset + 2, kind, little);
    u32_at(bytes, offset + 4, count, little);
    if kind == 3 && count == 1 {
        u16_at(bytes, offset + 8, value as u16, little);
    } else {
        u32_at(bytes, offset + 8, value, little);
    }
}
fn distant_tiff(little: bool) -> Vec<u8> {
    let mut bytes = vec![0; 3 * 1024 * 1024];
    bytes[..2].copy_from_slice(if little { b"II" } else { b"MM" });
    u16_at(&mut bytes, 2, 42, little);
    u32_at(&mut bytes, 4, 1024 * 1024, little);
    let first = 1024 * 1024;
    u16_at(&mut bytes, first, 5, little);
    for (index, tag, kind, count, value) in [
        (0, 0x0100, 4, 1, 1920),
        (1, 0x0101, 4, 1, 1080),
        (2, 0x010f, 2, 7, 2 * 1024 * 1024),
        (3, 0x0112, 3, 1, 6),
        (4, 0x8769, 4, 1, 1024 * 1024 + 256),
    ] {
        entry(
            &mut bytes,
            first + 2 + index * 12,
            tag,
            kind,
            count,
            value,
            little,
        );
    }
    let exif = first + 256;
    u16_at(&mut bytes, exif, 2, little);
    entry(&mut bytes, exif + 2, 0x8827, 3, 1, 320, little);
    entry(
        &mut bytes,
        exif + 14,
        0x9003,
        2,
        20,
        2 * 1024 * 1024 + 64,
        little,
    );
    bytes[2 * 1024 * 1024..2 * 1024 * 1024 + 7].copy_from_slice(b"Camera\0");
    bytes[2 * 1024 * 1024 + 64..2 * 1024 * 1024 + 84].copy_from_slice(b"2026:01:02 03:04:05\0");
    bytes
}
struct Counted {
    input: Cursor<Vec<u8>>,
    read: usize,
}
impl Read for Counted {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let count = self.input.read(output)?;
        self.read += count;
        Ok(count)
    }
}
impl Seek for Counted {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.input.seek(from)
    }
}

#[test]
fn far_tiff_metadata_is_projected_with_bounded_reads_in_both_byte_orders() {
    for little in [true, false] {
        let mut input = Counted {
            input: Cursor::new(distant_tiff(little)),
            read: 0,
        };
        let tiff = read(&mut input).unwrap().unwrap();
        let metadata = crate::utils::image_metadata::read(&tiff);
        assert_eq!(metadata.make, "Camera");
        assert_eq!(metadata.rotation, 90);
        assert_eq!(metadata.iso_speed, 320);
        assert_eq!(metadata.raw_taken_at, "2026:01:02 03:04:05");
        assert_eq!(
            crate::utils::image_dimensions::dimensions(&tiff),
            Some((1920, 1080))
        );
        assert!(
            input.read < 1024,
            "metadata reads must skip pixel ranges: {}",
            input.read
        );
        assert!(tiff.len() < 1024);
    }
}

#[test]
fn cyclic_and_oversized_tiff_fields_are_rejected_without_reading_the_file_body() {
    for oversized in [true, false] {
        let mut bytes = distant_tiff(true);
        if oversized {
            u32_at(&mut bytes, 1024 * 1024 + 2 + 2 * 12 + 4, u32::MAX, true);
        } else {
            u32_at(&mut bytes, 1024 * 1024 + 2 + 4 * 12 + 8, 1024 * 1024, true);
        }
        let mut input = Counted {
            input: Cursor::new(bytes),
            read: 0,
        };
        assert!(read(&mut input).is_err());
        assert!(input.read < 1024);
    }
}

#[test]
fn png_metadata_after_large_pixel_chunks_is_still_found() {
    let mut tiff = vec![0; 32];
    tiff[..2].copy_from_slice(b"II");
    u16_at(&mut tiff, 2, 42, true);
    u32_at(&mut tiff, 4, 8, true);
    u16_at(&mut tiff, 8, 1, true);
    entry(&mut tiff, 10, 0x0112, 3, 1, 6, true);
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    for (tag, body) in [
        (b"IDAT", vec![0; 256 * 1024]),
        (b"eXIf", tiff),
        (b"IEND", Vec::new()),
    ] {
        png.extend_from_slice(&(body.len() as u32).to_be_bytes());
        png.extend_from_slice(tag);
        png.extend_from_slice(&body);
        png.extend_from_slice(&[0; 4]);
    }
    let tiff = read(&mut Cursor::new(png)).unwrap().unwrap();
    assert_eq!(crate::utils::image_metadata::read(&tiff).rotation, 90);
}
