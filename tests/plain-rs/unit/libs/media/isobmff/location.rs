use super::*;
use std::io::Cursor;

fn atom(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut bytes = (8 + data.len() as u32).to_be_bytes().to_vec();
    bytes.extend(kind);
    bytes.extend(data);
    bytes
}

fn metadata(value: &str, full_box: bool) -> Vec<u8> {
    let key = b"com.apple.quicktime.location.ISO6709";
    let mut keys = vec![0; 4];
    keys.extend(1u32.to_be_bytes());
    keys.extend(atom(b"mdta", key));
    let mut data = 1u32.to_be_bytes().to_vec();
    data.extend([0; 4]);
    data.extend(value.as_bytes());
    let mut children = if full_box { vec![0; 4] } else { Vec::new() };
    children.extend(atom(b"keys", &keys));
    children.extend(atom(
        b"ilst",
        &atom(&1u32.to_be_bytes(), &atom(b"data", &data)),
    ));
    atom(b"meta", &children)
}

#[test]
fn location_reads_android_user_data_and_both_quicktime_metadata_headers() {
    let raw = "+12.345678-123.456789+14.5/";
    let mut data = (raw.len() as u16).to_be_bytes().to_vec();
    data.extend(0x15c7u16.to_be_bytes());
    data.extend(raw.as_bytes());
    let android = atom(b"moov", &atom(b"udta", &atom(b"\xa9xyz", &data)));
    assert_eq!(read(&mut Cursor::new(android)).as_deref(), Some(raw));
    for full_box in [true, false] {
        let movie = atom(b"moov", &metadata(raw, full_box));
        assert_eq!(read(&mut Cursor::new(movie)).as_deref(), Some(raw));
    }
}

struct Counted {
    reader: Cursor<Vec<u8>>,
    read: usize,
}
impl Read for Counted {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let count = self.reader.read(bytes)?;
        self.read += count;
        Ok(count)
    }
}
impl Seek for Counted {
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
        self.reader.seek(from)
    }
}

#[test]
fn location_seeks_past_media_and_unselected_metadata_payloads_without_reading_them() {
    let mut movie = atom(b"mdat", &vec![9; 4 * 1024 * 1024]);
    let mut children = atom(b"free", &vec![7; 1024 * 1024]);
    children.extend(metadata("+1.5-2.75/", false));
    movie.extend(atom(b"moov", &children));
    let mut reader = Counted {
        reader: Cursor::new(movie),
        read: 0,
    };
    assert_eq!(read(&mut reader).as_deref(), Some("+1.5-2.75/"));
    assert!(reader.read < 4096, "read {} bytes", reader.read);
}

#[test]
fn malformed_location_boxes_do_not_escape_their_boundaries() {
    let bad = atom(
        b"moov",
        &atom(b"udta", &atom(b"\xa9xyz", &[0xff, 0xff, 0, 0, b'+'])),
    );
    assert!(read(&mut Cursor::new(bad)).is_none());
    let mut bad = metadata("+1.0-2.0/", true);
    bad.truncate(bad.len() - 1);
    assert!(read(&mut Cursor::new(atom(b"moov", &bad))).is_none());
    assert!(read(&mut Cursor::new(atom(b"moov", &atom(b"meta", &[0])))).is_none());
}
