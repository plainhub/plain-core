use super::*;
use std::io::Cursor;

fn fixture(milliseconds: u64, preroll: u64, broadcast: bool) -> Vec<u8> {
    let mut body = vec![0; 80];
    body[40..48].copy_from_slice(
        &milliseconds
            .checked_add(preroll)
            .unwrap()
            .checked_mul(10000)
            .unwrap()
            .to_le_bytes(),
    );
    body[56..64].copy_from_slice(&preroll.to_le_bytes());
    body[64..68].copy_from_slice(&u32::from(broadcast).to_le_bytes());
    let mut header = HEADER.to_vec();
    header.extend(134u64.to_le_bytes());
    header.extend(1u32.to_le_bytes());
    header.extend([1, 2]);
    header.extend(FILE_PROPERTIES);
    header.extend(104u64.to_le_bytes());
    header.extend(body);
    header
}

#[test]
fn file_properties_convert_100ns_units_remove_preroll_and_preserve_large_durations() {
    for duration in [0, 3211, 5_000_000_001] {
        let bytes = fixture(duration, 3000, false);
        assert_eq!(
            milliseconds(&mut Cursor::new(&bytes)),
            Some(duration as i64)
        );
        assert_eq!(
            crate::libs::media::probe::probe_duration_reader(&mut Cursor::new(bytes)),
            Some(duration as i64)
        );
    }
}

#[test]
fn broadcast_malformed_sizes_and_truncated_objects_remain_unknown() {
    assert_eq!(milliseconds(&mut Cursor::new(fixture(1000, 0, true))), None);
    let valid = fixture(3211, 3000, false);
    for cut in [0, 16, 29, 30, 53, 133] {
        assert_eq!(milliseconds(&mut Cursor::new(&valid[..cut])), None);
    }
    let mut overflow = valid.clone();
    overflow[46..54].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(milliseconds(&mut Cursor::new(overflow)), None);
    let mut huge_count = valid;
    huge_count[24..28].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(milliseconds(&mut Cursor::new(huge_count)), None);
}

#[test]
fn file_properties_seek_over_unselected_objects_and_stay_within_descriptor_bounds() {
    let mut bytes = fixture(3211, 3000, false);
    let properties = bytes.split_off(30);
    let padding = 1024 * 1024usize;
    bytes[16..24].copy_from_slice(&(134 + 24 + padding as u64).to_le_bytes());
    bytes[24..28].copy_from_slice(&2u32.to_le_bytes());
    bytes.extend([7; 16]);
    bytes.extend((24 + padding as u64).to_le_bytes());
    bytes.resize(bytes.len() + padding, 0);
    bytes.extend(properties);
    let mut wrapped = b"outside".to_vec();
    let offset = wrapped.len() as u64;
    wrapped.extend_from_slice(&bytes);
    wrapped.extend(b"outside");
    let mut counted = Counted {
        reader: Cursor::new(wrapped),
        read: 0,
    };
    let mut range = crate::libs::utils::range_reader::RangeReader::new(
        &mut counted,
        offset,
        bytes.len() as u64,
    )
    .unwrap();
    assert_eq!(milliseconds(&mut range), Some(3211));
    drop(range);
    assert!(counted.read < 1024, "read {} bytes", counted.read);
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

fn object(guid: [u8; 16], data: &[u8]) -> Vec<u8> {
    [
        guid.to_vec(),
        (24 + data.len() as u64).to_le_bytes().to_vec(),
        data.to_vec(),
    ]
    .concat()
}
fn with_objects(mut header: Vec<u8>, objects: &[Vec<u8>]) -> Vec<u8> {
    let count = u32_at(&header, 24).unwrap() + objects.len() as u32;
    for object in objects {
        header.extend(object);
    }
    let length = header.len() as u64;
    header[16..24].copy_from_slice(&length.to_le_bytes());
    header[24..28].copy_from_slice(&count.to_le_bytes());
    header
}
fn wide(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(u16::to_le_bytes)
        .collect()
}
fn description() -> Vec<u8> {
    let values = [
        wide("ASF Title"),
        wide("ASF Artist"),
        wide(""),
        wide(""),
        wide(""),
    ];
    let mut bytes = Vec::new();
    for value in &values {
        bytes.extend((value.len() as u16).to_le_bytes());
    }
    for value in values {
        bytes.extend(value);
    }
    object(CONTENT, &bytes)
}
fn descriptor(name: &str, kind: u16, value: &[u8]) -> Vec<u8> {
    let name = wide(name);
    [
        (name.len() as u16).to_le_bytes().to_vec(),
        name,
        kind.to_le_bytes().to_vec(),
        (value.len() as u16).to_le_bytes().to_vec(),
        value.to_vec(),
    ]
    .concat()
}
fn stream() -> Vec<u8> {
    let mut bytes = vec![0; 54 + 51];
    bytes[..16].copy_from_slice(&VIDEO_MEDIA);
    bytes[40..44].copy_from_slice(&51u32.to_le_bytes());
    bytes[48..50].copy_from_slice(&1u16.to_le_bytes());
    bytes[54..58].copy_from_slice(&1920u32.to_le_bytes());
    bytes[58..62].copy_from_slice(&1080u32.to_le_bytes());
    bytes[63..65].copy_from_slice(&40u16.to_le_bytes());
    bytes[65..69].copy_from_slice(&40u32.to_le_bytes());
    bytes[69..73].copy_from_slice(&640i32.to_le_bytes());
    bytes[73..77].copy_from_slice(&(-360i32).to_le_bytes());
    object(STREAM_PROPERTIES, &bytes)
}
#[test]
fn video_and_audio_metadata_share_utf16_tags_and_coded_dimensions() {
    let records = [
        descriptor("WM/AlbumTitle", 0, &wide("Album")),
        descriptor("WM/AlbumArtist", 0, &wide("Fallback Artist")),
        descriptor("WM/Genre", 0, &wide("Genre")),
        descriptor("WM/Composer", 0, &wide("Composer")),
        descriptor("WM/VideoFrameRate", 0, &wide("29.97")),
    ];
    let bytes = with_objects(
        fixture(3211, 3000, false),
        &[
            description(),
            object(
                EXTENDED_CONTENT,
                &[5u16.to_le_bytes().to_vec(), records.concat()].concat(),
            ),
            stream(),
        ],
    );
    let facts = crate::libs::media::video_info::read(&mut Cursor::new(&bytes)).unwrap();
    assert_eq!((facts.width, facts.height), (640, 360));
    assert_eq!(facts.duration_ms, 3211);
    assert_eq!(facts.title, "ASF Title");
    assert_eq!(facts.artist, "ASF Artist");
    assert_eq!(facts.album, "Album");
    assert_eq!(facts.composer, "Composer");
    assert_eq!(facts.frame_rate, 29.97);
    let audio = crate::libs::media::probe::probe_audio_reader(&mut Cursor::new(bytes));
    assert!(audio.parsed && audio.duration_available);
    assert_eq!(audio.duration_ms, 3211);
    assert_eq!(audio.title, "ASF Title");
    assert_eq!(audio.artist, "ASF Artist");
    assert_eq!(audio.album, "Album");
    assert_eq!(audio.album_artist, "Fallback Artist");
}
#[test]
fn metadata_skips_binary_descriptors_and_unknown_objects_without_reading_media() {
    let binary = descriptor("WM/Picture", 1, &vec![7; 65534]);
    let text = descriptor("WM/Genre", 0, &wide("After Picture"));
    let bytes = with_objects(
        fixture(3211, 3000, false),
        &[
            object([9; 16], &vec![8; 1024 * 1024]),
            object(
                EXTENDED_CONTENT,
                &[2u16.to_le_bytes().to_vec(), binary, text].concat(),
            ),
            stream(),
        ],
    );
    let length = bytes.len();
    let mut wrapped = b"outside".to_vec();
    wrapped.extend(bytes);
    wrapped.extend(b"outside");
    let mut counted = Counted {
        reader: Cursor::new(wrapped),
        read: 0,
    };
    let mut range =
        crate::libs::utils::range_reader::RangeReader::new(&mut counted, 7, length as u64).unwrap();
    assert_eq!(video(&mut range).unwrap().genre, "After Picture");
    drop(range);
    assert!(counted.read < 4096, "read {} bytes", counted.read);
}
#[test]
fn invalid_stream_lengths_and_utf16_are_bounded_without_losing_duration() {
    let mut invalid = stream();
    invalid[24 + 40..24 + 44].copy_from_slice(&u32::MAX.to_le_bytes());
    let bytes = with_objects(fixture(3211, 3000, false), &[invalid]);
    assert!(video(&mut Cursor::new(&bytes)).is_none());
    assert_eq!(milliseconds(&mut Cursor::new(bytes)), Some(3211));
    assert_eq!(tags::utf16(&[0]), None);
    assert_eq!(tags::utf16(&[0, 0xd8]), None);
    let broadcast = with_objects(fixture(1000, 0, true), &[description(), stream()]);
    let audio = crate::libs::media::probe::probe_audio_reader(&mut Cursor::new(&broadcast));
    assert!(audio.parsed);
    assert!(!audio.duration_available);
    assert_eq!(audio.title, "ASF Title");
    assert_eq!(milliseconds(&mut Cursor::new(broadcast)), None);
}
#[test]
fn generated_wmv_and_wma_use_the_same_portable_reader() {
    let bytes = include_bytes!("../../../../../plain-rs/testdata/video-wmv2.wmv");
    let result = video(&mut Cursor::new(bytes)).unwrap();
    assert_eq!((result.width, result.height), (16, 16));
    assert_eq!(result.duration_ms, 120);
    assert_eq!(result.frame_rate, 25.0);
    assert_eq!(result.title, "WmvFixture");
    assert_eq!(result.artist, "FixtureArtist");
    let bytes = include_bytes!("../../../../../plain-rs/testdata/audio-wmav2.wma");
    let audio = crate::libs::media::probe::probe_audio_reader(&mut Cursor::new(bytes));
    assert!(audio.parsed && audio.duration_available);
    assert!(audio.duration_ms > 0);
    assert_eq!(audio.title, "WmaFixture");
    assert_eq!(audio.artist, "FixtureArtist");
}

fn metadata_record(stream: u16, name: &str, kind: u16, value: &[u8]) -> Vec<u8> {
    let name = wide(name);
    [
        0u16.to_le_bytes().to_vec(),
        stream.to_le_bytes().to_vec(),
        (name.len() as u16).to_le_bytes().to_vec(),
        kind.to_le_bytes().to_vec(),
        (value.len() as u32).to_le_bytes().to_vec(),
        name,
        value.to_vec(),
    ]
    .concat()
}
fn extension(children: &[Vec<u8>]) -> Vec<u8> {
    let children = children.concat();
    let reserved = [
        0x11, 0xd2, 0xd3, 0xab, 0xba, 0xa9, 0xcf, 0x11, 0x8e, 0xe6, 0, 0xc0, 0x0c, 0x20, 0x53, 0x65,
    ];
    object(
        objects::HEADER_EXTENSION,
        &[
            reserved.to_vec(),
            6u16.to_le_bytes().to_vec(),
            (children.len() as u32).to_le_bytes().to_vec(),
            children,
        ]
        .concat(),
    )
}
fn library(records: &[Vec<u8>]) -> Vec<u8> {
    object(
        METADATA_LIBRARY,
        &[
            (records.len() as u16).to_le_bytes().to_vec(),
            records.concat(),
        ]
        .concat(),
    )
}
fn picture(kind: u8, bytes: &[u8]) -> Vec<u8> {
    [
        vec![kind],
        (bytes.len() as u32).to_le_bytes().to_vec(),
        wide("image/jpeg"),
        wide("Artwork"),
        bytes.to_vec(),
    ]
    .concat()
}
#[test]
fn header_extension_metadata_is_shared_by_video_and_audio_and_ignores_other_streams() {
    let bytes = with_objects(
        fixture(3211, 3000, false),
        &[
            stream(),
            extension(&[library(&[
                metadata_record(0, "Title", 0, &wide("Library title")),
                metadata_record(0, "Author", 0, &wide("Library artist")),
                metadata_record(0, "WM/AlbumTitle", 0, &wide("Album")),
                metadata_record(2, "Title", 0, &wide("Other stream")),
            ])]),
        ],
    );
    let facts = video(&mut Cursor::new(&bytes)).unwrap();
    assert_eq!(facts.title, "Library title");
    assert_eq!(facts.artist, "Library artist");
    assert_eq!(facts.album, "Album");
    let audio = crate::libs::media::probe::probe_audio_reader(&mut Cursor::new(bytes));
    assert!(audio.parsed && audio.duration_available);
    assert_eq!(audio.title, "Library title");
    assert_eq!(audio.artist, "Library artist");
    assert_eq!(audio.duration_ms, 3211);
}
#[test]
fn embedded_artwork_prefers_front_cover_across_description_and_metadata_library() {
    let other = include_bytes!("../../../../../plain-rs/testdata/v-prog-gray.jpg");
    let front = include_bytes!("../../../../../plain-rs/testdata/v-prog420.jpg");
    let bytes = with_objects(
        fixture(0, 0, false),
        &[
            object(
                EXTENDED_CONTENT,
                &[
                    1u16.to_le_bytes().to_vec(),
                    descriptor("WM/Picture", 1, &picture(0, other)),
                ]
                .concat(),
            ),
            extension(&[library(&[metadata_record(
                0,
                "WM/Picture",
                1,
                &picture(3, front),
            )])]),
        ],
    );
    let cover =
        crate::libs::media::cover::extract_embedded_cover_reader(&mut Cursor::new(&bytes)).unwrap();
    assert_eq!(cover.mime, "image/jpeg");
    assert_eq!(cover.bytes, front);
    assert_eq!(milliseconds(&mut Cursor::new(bytes)), Some(0));
}
#[test]
fn large_unselected_library_values_are_skipped_in_metadata_and_owned_range_cover_reads() {
    let image = include_bytes!("../../../../../plain-rs/testdata/v-prog-gray.jpg");
    let bytes = with_objects(
        fixture(3211, 0, false),
        &[extension(&[library(&[
            metadata_record(0, "Unselected", 1, &vec![7; 5 * 1024 * 1024]),
            metadata_record(0, "Title", 0, &wide("After binary")),
            metadata_record(0, "WM/Picture", 1, &picture(3, image)),
        ])])],
    );
    let mut source = Counted {
        reader: Cursor::new(bytes.clone()),
        read: 0,
    };
    assert_eq!(metadata(&mut source).unwrap().0.title, "After binary");
    assert!(source.read < 4096, "{}", source.read);
    let mut wrapped = b"outside".to_vec();
    wrapped.extend(&bytes);
    wrapped.extend(b"outside");
    let mut source = Counted {
        reader: Cursor::new(wrapped),
        read: 0,
    };
    let mut range =
        crate::libs::utils::range_reader::RangeReader::new(&mut source, 7, bytes.len() as u64)
            .unwrap();
    let cover = crate::libs::media::cover::extract_embedded_cover_reader(&mut range).unwrap();
    assert_eq!(cover.bytes, image);
    drop(range);
    assert!(source.read < image.len() + 24 * 1024, "{}", source.read);
}
#[test]
fn invalid_picture_lengths_and_strings_do_not_hide_later_valid_artwork() {
    let image = include_bytes!("../../../../../plain-rs/testdata/v-prog-gray.jpg");
    let mut oversized = picture(3, image);
    oversized[1..5].copy_from_slice(&u32::MAX.to_le_bytes());
    let unterminated = [vec![3], 1u32.to_le_bytes().to_vec(), vec![65; 32]].concat();
    let bytes = with_objects(
        fixture(0, 0, false),
        &[extension(&[library(&[
            metadata_record(0, "WM/Picture", 1, &oversized),
            metadata_record(0, "WM/Picture", 1, &unterminated),
            metadata_record(0, "WM/Picture", 1, &picture(0, image)),
        ])])],
    );
    assert_eq!(cover(&mut Cursor::new(bytes)).unwrap().bytes, image);
}
#[test]
fn malformed_extensions_nested_objects_and_incomplete_counts_are_rejected() {
    let mut malformed = extension(&[]);
    malformed[24 + 18..24 + 22].copy_from_slice(&u32::MAX.to_le_bytes());
    let bytes = with_objects(fixture(0, 0, false), &[malformed]);
    assert!(metadata(&mut Cursor::new(&bytes)).is_none());
    assert!(cover(&mut Cursor::new(&bytes)).is_none());
    assert_eq!(milliseconds(&mut Cursor::new(bytes)), Some(0));
    let bytes = with_objects(fixture(0, 0, false), &[extension(&[extension(&[])])]);
    assert!(metadata(&mut Cursor::new(bytes)).is_none());
    let mut bytes = fixture(0, 0, false);
    bytes[24..28].copy_from_slice(&2u32.to_le_bytes());
    assert!(metadata(&mut Cursor::new(bytes)).is_none());
}
#[test]
fn text_budget_rejects_excessive_selected_values_without_reading_all_of_them() {
    let record = metadata_record(0, "Title", 0, &wide(&"x".repeat(1024)));
    let bytes = with_objects(
        fixture(0, 0, false),
        &[extension(&[library(&vec![record; 3000])])],
    );
    let mut source = Counted {
        reader: Cursor::new(bytes),
        read: 0,
    };
    assert!(metadata(&mut source).is_none());
    assert!(source.read <= 4 * 1024 * 1024 + 256, "{}", source.read);
}

#[test]
fn metadata_library_artwork_accepts_32_bit_lengths_and_prefers_other_to_back_cover() {
    let mut image = include_bytes!("../../../../../plain-rs/testdata/v-prog-gray.jpg").to_vec();
    image.resize(70_000, 0);
    let bytes = with_objects(
        fixture(0, 0, false),
        &[extension(&[library(&[
            metadata_record(0, "WM/Picture", 1, &picture(4, b"back cover")),
            metadata_record(0, "WM/Picture", 1, &picture(0, &image)),
            metadata_record(2, "WM/Picture", 1, &picture(3, b"foreign front")),
        ])])],
    );
    let cover =
        crate::libs::media::cover::extract_embedded_cover_reader(&mut Cursor::new(bytes)).unwrap();
    assert_eq!(cover.bytes, image);
}
#[test]
fn generated_wma_library_fixture_preserves_audio_metadata_duration_and_embedded_image() {
    let bytes = include_bytes!("../../../../../plain-rs/testdata/audio-asf-library-cover.wma");
    let audio = crate::libs::media::probe::probe_audio_reader(&mut Cursor::new(bytes));
    assert!(audio.parsed && audio.duration_available);
    assert_eq!(audio.title, "ASF fixture title");
    assert_eq!(audio.artist, "ASF fixture artist");
    let original = crate::libs::media::probe::probe_audio_reader(&mut Cursor::new(include_bytes!(
        "../../../../../plain-rs/testdata/audio-wmav2.wma"
    )));
    assert_eq!(audio.duration_ms, original.duration_ms);
    let picture =
        crate::libs::media::cover::extract_embedded_cover_reader(&mut Cursor::new(bytes)).unwrap();
    assert_eq!(
        picture.bytes,
        include_bytes!("../../../../../plain-rs/testdata/v-prog-gray.jpg")
    );
}
