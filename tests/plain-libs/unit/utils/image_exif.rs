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

fn orientation_tiff() -> Vec<u8> {
    let mut tiff = vec![0; 26];
    tiff[..4].copy_from_slice(b"II\x2a\0");
    u32_at(&mut tiff, 4, 8, true);
    u16_at(&mut tiff, 8, 1, true);
    entry(&mut tiff, 10, 0x0112, 3, 1, 6, true);
    tiff
}

fn png(tiff: &[u8]) -> Vec<u8> {
    let mut image = b"\x89PNG\r\n\x1a\n".to_vec();
    image.extend_from_slice(&(tiff.len() as u32).to_be_bytes());
    image.extend_from_slice(b"eXIf");
    image.extend_from_slice(tiff);
    image.extend_from_slice(&[0; 4]);
    image
}

fn jpeg(tiff: &[u8]) -> Vec<u8> {
    let mut image = b"\xff\xd8\xff\xe1".to_vec();
    image.extend_from_slice(&((tiff.len() + 8) as u16).to_be_bytes());
    image.extend_from_slice(b"Exif\0\0");
    image.extend_from_slice(tiff);
    image.extend_from_slice(b"\xff\xd9");
    image
}

fn webp(tiff: &[u8]) -> Vec<u8> {
    let mut image = b"RIFF".to_vec();
    image.extend_from_slice(&((tiff.len() + 12 + tiff.len() % 2) as u32).to_le_bytes());
    image.extend_from_slice(b"WEBPEXIF");
    image.extend_from_slice(&(tiff.len() as u32).to_le_bytes());
    image.extend_from_slice(tiff);
    if tiff.len() % 2 != 0 {
        image.push(0);
    }
    image
}

fn heif(tiff: &[u8], construction: u16) -> Vec<u8> {
    fn atom(tag: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut output = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        output.extend_from_slice(tag);
        output.extend_from_slice(body);
        output
    }
    let ftyp = atom(b"ftyp", b"mif1\0\0\0\0mif1");
    let mut payload = vec![0; 4];
    payload.extend_from_slice(tiff);
    let make_meta = |offset: u32| {
        let infe = atom(b"infe", b"\x02\0\0\0\0\x01\0\0Exif");
        let mut iinf = vec![0, 0, 0, 0, 0, 1];
        iinf.extend_from_slice(&infe);
        let mut iloc = vec![1, 0, 0, 0, 0x44, 0, 0, 1, 0, 1];
        iloc.extend_from_slice(&construction.to_be_bytes());
        iloc.extend_from_slice(&[0, 0, 0, 1]);
        iloc.extend_from_slice(&offset.to_be_bytes());
        iloc.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        let mut meta = vec![0; 4];
        meta.extend_from_slice(&atom(b"iinf", &iinf));
        meta.extend_from_slice(&atom(b"iloc", &iloc));
        if construction == 1 {
            meta.extend_from_slice(&atom(b"idat", &payload));
        }
        atom(b"meta", &meta)
    };
    let meta = make_meta(0);
    let offset = if construction == 0 {
        (ftyp.len() + meta.len() + 8) as u32
    } else {
        0
    };
    let mut image = ftyp;
    image.extend_from_slice(&make_meta(offset));
    if construction == 0 {
        image.extend_from_slice(&atom(b"mdat", &payload));
    }
    image
}

#[test]
fn all_retained_containers_return_the_same_tiff_and_orientation() {
    let tiff = orientation_tiff();
    for image in [
        jpeg(&tiff),
        png(&tiff),
        webp(&tiff),
        heif(&tiff, 0),
        heif(&tiff, 1),
    ] {
        let extracted = read(&mut Cursor::new(&image)).unwrap().unwrap();
        assert_eq!(extracted, tiff);
        assert_eq!(crate::utils::image_metadata::read(&extracted).rotation, 90);
        for end in 0..image.len() {
            let result = read(&mut Cursor::new(&image[..end]));
            if let Ok(Some(extracted)) = result {
                assert_eq!(extracted, tiff);
            }
        }
    }
}

#[test]
fn corrupt_tiff_in_containers_is_rejected_instead_of_being_projected() {
    let mut tiff = orientation_tiff();
    u16_at(&mut tiff, 12, 4, true);
    u32_at(&mut tiff, 14, u32::MAX, true);
    for image in [
        jpeg(&tiff),
        png(&tiff),
        webp(&tiff),
        heif(&tiff, 0),
        heif(&tiff, 1),
    ] {
        assert!(read(&mut Cursor::new(image)).is_err());
    }
}

#[test]
fn oversized_metadata_is_rejected_before_payload_read() {
    let mut image = b"\x89PNG\r\n\x1a\n".to_vec();
    image.extend_from_slice(&((MAX_METADATA_BYTES + 1) as u32).to_be_bytes());
    image.extend_from_slice(b"eXIf");
    let mut reader = Counted {
        input: Cursor::new(image),
        read: 0,
    };
    let error = read(&mut reader).unwrap_err();
    assert!(error.to_string().contains("too large"));
    assert_eq!(reader.read, 24);
}

#[test]
fn missing_png_metadata_stops_at_eof() {
    struct EofBudget {
        input: Cursor<Vec<u8>>,
        calls: usize,
    }
    impl Read for EofBudget {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            self.calls += 1;
            assert!(self.calls < 16, "PNG EOF must terminate parsing");
            self.input.read(bytes)
        }
    }
    impl Seek for EofBudget {
        fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
            self.input.seek(position)
        }
    }
    let mut image = b"\x89PNG\r\n\x1a\n".to_vec();
    image.extend_from_slice(b"\0\0\0\0IEND\0\0\0\0");
    let mut reader = EofBudget {
        input: Cursor::new(image),
        calls: 0,
    };
    assert!(read(&mut reader).unwrap().is_none());
}

#[test]
fn heif_zero_size_metadata_boxes_keep_eof_semantics_and_enforce_the_read_limit() {
    let tiff = orientation_tiff();
    let mut image = heif(&tiff, 1);
    assert_eq!(&image[24..28], b"meta");
    image[20..24].copy_from_slice(&0u32.to_be_bytes());
    assert_eq!(read(&mut Cursor::new(&image)).unwrap().unwrap(), tiff);
    image.resize(2 * MAX_METADATA_BYTES, 0);
    let mut reader = Counted {
        input: Cursor::new(image),
        read: 0,
    };
    assert!(
        read(&mut reader)
            .unwrap_err()
            .to_string()
            .contains("too large")
    );
    assert!(reader.read < reader.input.get_ref().len());
}

#[test]
fn heif_zero_length_extents_preserve_eof_and_idat_extraction() {
    let tiff = orientation_tiff();
    for construction in [0, 1] {
        let mut image = heif(&tiff, construction);
        let location = image.windows(4).position(|part| part == b"iloc").unwrap();
        image[location + 24..location + 28].copy_from_slice(&0u32.to_be_bytes());
        assert_eq!(read(&mut Cursor::new(image)).unwrap().unwrap(), tiff);
    }
}
