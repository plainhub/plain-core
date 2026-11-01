use super::*;
use std::io::Cursor;

fn atom(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut bytes = (8 + data.len() as u32).to_be_bytes().to_vec();
    bytes.extend(kind);
    bytes.extend(data);
    bytes
}
fn metadata_item(kind: &[u8; 4], value: &[u8], encoding: u32) -> Vec<u8> {
    let mut data = encoding.to_be_bytes().to_vec();
    data.extend([0; 4]);
    data.extend(value);
    atom(kind, &atom(b"data", &data))
}
fn fixture(version: u8, rotation: i32, capture_rate: bool) -> Vec<u8> {
    let matrix = if version == 0 { 40 } else { 52 };
    let mut tkhd = vec![0; matrix + 44];
    tkhd[0] = version;
    let id = if version == 0 { 12 } else { 20 };
    tkhd[id..id + 4].copy_from_slice(&1u32.to_be_bytes());
    let (a, b): (i32, i32) = match rotation {
        90 => (0, 65536),
        180 => (-65536, 0),
        270 => (0, -65536),
        _ => (65536, 0),
    };
    tkhd[matrix..matrix + 4].copy_from_slice(&a.to_be_bytes());
    tkhd[matrix + 4..matrix + 8].copy_from_slice(&b.to_be_bytes());
    tkhd[matrix + 36..matrix + 40].copy_from_slice(&(1920u32 << 16).to_be_bytes());
    tkhd[matrix + 40..matrix + 44].copy_from_slice(&(1080u32 << 16).to_be_bytes());
    let mut time = vec![0; 12];
    time.extend(30000u32.to_be_bytes());
    time.extend(30030u32.to_be_bytes());
    let mut handler = vec![0; 8];
    handler.extend(b"vide");
    let mut sample = vec![0; 78];
    sample[24..26].copy_from_slice(&640u16.to_be_bytes());
    sample[26..28].copy_from_slice(&360u16.to_be_bytes());
    let mut stsd = vec![0; 4];
    stsd.extend(1u32.to_be_bytes());
    stsd.extend(atom(b"avc1", &sample));
    let mut stts = vec![0; 4];
    stts.extend(1u32.to_be_bytes());
    stts.extend(30u32.to_be_bytes());
    stts.extend(1001u32.to_be_bytes());
    let table = atom(
        b"stbl",
        &[atom(b"stsd", &stsd), atom(b"stts", &stts)].concat(),
    );
    let media = atom(
        b"mdia",
        &[
            atom(b"mdhd", &time),
            atom(b"hdlr", &handler),
            atom(b"minf", &table),
        ]
        .concat(),
    );
    let mut items = metadata_item(b"\xa9nam", b"Sample Title", 1);
    items.extend(metadata_item(b"\xa9ART", b"Sample Artist", 1));
    items.extend(metadata_item(b"\xa9wrt", b"Sample Composer", 1));
    items.extend(metadata_item(b"gnre", &6u16.to_be_bytes(), 0));
    items.extend(metadata_item(b"\xa9day", b"2026-10-16T12:34:56+08:00", 1));
    let mut tags = vec![0; 4];
    tags.extend(atom(b"ilst", &items));
    let mut children = vec![
        atom(b"mvhd", &time),
        atom(b"trak", &[atom(b"tkhd", &tkhd), media].concat()),
        atom(b"udta", &atom(b"meta", &tags)),
    ];
    if capture_rate {
        let mut keys = vec![0; 4];
        keys.extend(1u32.to_be_bytes());
        keys.extend(atom(b"mdta", b"com.android.capture.fps"));
        let mut data = atom(b"keys", &keys);
        data.extend(atom(
            b"ilst",
            &metadata_item(&1u32.to_be_bytes(), &240f32.to_bits().to_be_bytes(), 23),
        ));
        children.push(atom(b"meta", &data));
    }
    atom(b"moov", &children.concat())
}

#[test]
fn video_track_versions_preserve_coded_dimensions_rotation_fractional_rate_and_tags() {
    for version in [0, 1] {
        for rotation in [0, 90, 180, 270] {
            let bytes = fixture(version, rotation, false);
            let info = read(&mut Cursor::new(&bytes)).unwrap();
            assert_eq!((info.width, info.height), (640, 360));
            assert_eq!(info.rotation, rotation);
            assert_eq!(info.duration_ms, 1001);
            assert!((info.frame_rate - 29.97002997002997).abs() < 1e-9);
            assert_eq!(info.title, "Sample Title");
            assert_eq!(info.artist, "Sample Artist");
            assert_eq!(info.composer, "Sample Composer");
            assert_eq!(info.writer, "");
            assert_eq!(info.genre, "Funk");
            assert_eq!(
                info.raw_taken_at.as_deref(),
                Some("2026-10-16T12:34:56+08:00")
            );
            assert_eq!(info.bitrate, (bytes.len() as i64 * 8000) / 1001);
        }
    }
}

#[test]
fn capture_rate_overrides_playback_sample_average_and_corrupt_boxes_are_unknown() {
    let bytes = fixture(0, 90, true);
    assert_eq!(read(&mut Cursor::new(&bytes)).unwrap().frame_rate, 240.0);
    for cut in [0, 8, bytes.len() - 1] {
        assert!(read(&mut Cursor::new(&bytes[..cut])).is_none());
    }
    assert!(read(&mut Cursor::new(b"unknown video")).is_none());
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
fn metadata_stays_inside_the_descriptor_and_skips_media_payload_bytes() {
    let mut video = atom(b"mdat", &vec![7; 4 * 1024 * 1024]);
    video.extend(fixture(1, 270, false));
    let mut wrapped = b"outside".to_vec();
    let offset = wrapped.len() as u64;
    wrapped.extend_from_slice(&video);
    wrapped.extend(b"outside");
    let mut counted = Counted {
        reader: Cursor::new(wrapped),
        read: 0,
    };
    let mut range =
        crate::utils::range_reader::RangeReader::new(&mut counted, offset, video.len() as u64)
            .unwrap();
    let info = read(&mut range).unwrap();
    assert_eq!((info.width, info.height, info.rotation), (640, 360, 270));
    drop(range);
    assert!(counted.read < 32 * 1024, "read {} bytes", counted.read);
}

#[test]
fn unselected_cover_art_and_padding_inside_moov_are_not_read_or_projected() {
    let base = fixture(0, 0, false);
    let mut children = base[8..].to_vec();
    let mut large = vec![0; 8];
    large.extend(vec![7; 2 * 1024 * 1024]);
    let mut meta = vec![0; 4];
    meta.extend(atom(b"ilst", &atom(b"covr", &atom(b"data", &large))));
    children.extend(atom(b"udta", &atom(b"meta", &meta)));
    children.extend(atom(b"free", &vec![9; 2 * 1024 * 1024]));
    let mut reader = Counted {
        reader: Cursor::new(atom(b"moov", &children)),
        read: 0,
    };
    let info = read(&mut reader).unwrap();
    assert_eq!(info.title, "Sample Title");
    assert_eq!((info.width, info.height), (640, 360));
    assert!(reader.read < 32 * 1024, "read {} bytes", reader.read);
}

#[test]
fn quicktime_named_utf16_writer_metadata_is_preserved() {
    let base = fixture(0, 0, false);
    let mut children = base[8..].to_vec();
    let mut keys = vec![0; 4];
    keys.extend(1u32.to_be_bytes());
    keys.extend(atom(b"mdta", b"com.apple.quicktime.writer"));
    let writer: Vec<u8> = "Sample Writer"
        .encode_utf16()
        .flat_map(u16::to_be_bytes)
        .collect();
    let mut meta = atom(b"keys", &keys);
    meta.extend(atom(
        b"ilst",
        &metadata_item(&1u32.to_be_bytes(), &writer, 2),
    ));
    children.extend(atom(b"meta", &meta));
    let info = read(&mut Cursor::new(atom(b"moov", &children))).unwrap();
    assert_eq!(info.writer, "Sample Writer");
    assert_eq!(info.composer, "Sample Composer");
}

#[test]
fn three_gpp_user_data_uses_language_prefixed_strings_without_losing_track_numbers() {
    let base = fixture(0, 0, false);
    let mut children = Vec::new();
    for item in atoms(&base[8..]).unwrap() {
        if &item.kind != b"udta" {
            children.extend(atom(&item.kind, item.data));
        }
    }
    let mut tags = Vec::new();
    for (kind, value) in [
        (b"titl", b"3GP Title".as_slice()),
        (b"perf", b"3GP Artist".as_slice()),
        (b"albm", b"3GP Album\0\x07".as_slice()),
        (b"gnre", b"Rock".as_slice()),
    ] {
        let mut data = vec![0; 4];
        data.extend(0x15c7u16.to_be_bytes());
        data.extend(value);
        data.push(0);
        tags.extend(atom(kind, &data));
    }
    children.extend(atom(b"udta", &tags));
    let info = read(&mut Cursor::new(atom(b"moov", &children))).unwrap();
    assert_eq!(info.title, "3GP Title");
    assert_eq!(info.artist, "3GP Artist");
    assert_eq!(info.album, "3GP Album");
    assert_eq!(info.genre, "Rock");
}
