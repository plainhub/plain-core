use super::*;
use std::io::Cursor;

fn chunk(id: &[u8; 4], bytes: &[u8]) -> Vec<u8> {
    let mut result = id.to_vec();
    result.extend((bytes.len() as u32).to_le_bytes());
    result.extend(bytes);
    if bytes.len() % 2 != 0 {
        result.push(0);
    }
    result
}
fn list(kind: &[u8; 4], children: &[u8]) -> Vec<u8> {
    chunk(b"LIST", &[kind.as_slice(), children].concat())
}
fn document(children: &[u8]) -> Vec<u8> {
    chunk(b"RIFF", &[b"AVI ".as_slice(), children].concat())
}
fn main_header(frames: u32, microseconds: u32) -> Vec<u8> {
    let mut bytes = vec![0; 56];
    for (offset, value) in [(0, microseconds), (16, frames), (32, 1920), (36, 1080)] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    chunk(b"avih", &bytes)
}
fn stream(kind: &[u8; 4], flags: u32, frames: u32, scale: u32, rate: u32) -> Vec<u8> {
    let mut header = vec![0; 56];
    header[..4].copy_from_slice(kind);
    for (offset, value) in [(8, flags), (20, scale), (24, rate), (32, frames)] {
        header[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    let mut format = vec![0; 40];
    format[..4].copy_from_slice(&40u32.to_le_bytes());
    format[4..8].copy_from_slice(&640i32.to_le_bytes());
    format[8..12].copy_from_slice(&(-360i32).to_le_bytes());
    list(
        b"strl",
        &[chunk(b"strh", &header), chunk(b"strf", &format)].concat(),
    )
}
fn headers() -> Vec<u8> {
    [
        main_header(3, 33367),
        stream(b"auds", 0, 8000, 1, 8000),
        stream(b"vids", 1, 1, 1, 1),
        stream(b"vids", 0, 3, 1001, 30000),
    ]
    .concat()
}
#[test]
fn stream_timing_and_coded_top_down_dimensions_override_coarse_main_values() {
    let bytes = document(
        &[
            list(b"hdrl", &headers()),
            chunk(b"JUNK", &[1, 2, 3]),
            list(
                b"INFO",
                &[
                    chunk(b"INAM", b"AVI Title\0"),
                    chunk(b"IART", b"Artist\0"),
                    chunk(b"IGNR", b"Genre\0"),
                    chunk(b"ICRD", b"2026-10-16T00:00:00Z\0"),
                ]
                .concat(),
            ),
        ]
        .concat(),
    );
    let video = super::super::video_info::read(&mut Cursor::new(&bytes)).unwrap();
    assert_eq!((video.width, video.height), (640, 360));
    assert_eq!(video.duration_ms, 100);
    assert!((video.frame_rate - 29.97002997002997).abs() < 1e-9);
    assert_eq!(video.title, "AVI Title");
    assert_eq!(video.artist, "Artist");
    assert_eq!(video.genre, "Genre");
    assert_eq!(video.raw_taken_at.as_deref(), Some("2026-10-16T00:00:00Z"));
    assert_eq!(
        super::super::probe::probe_duration_reader(&mut Cursor::new(bytes)),
        Some(100)
    );
}
#[test]
fn open_dml_total_frames_preserve_fractional_stream_time_base() {
    let frames = 1_234_567u32;
    let mut headers = headers();
    headers.extend(list(b"odml", &chunk(b"dmlh", &frames.to_le_bytes())));
    let bytes = document(&list(b"hdrl", &headers));
    assert_eq!(
        milliseconds(&mut Cursor::new(&bytes)),
        Some(i64::from(frames) * 1001 * 1000 / 30000)
    );
}
struct Counted {
    inner: Cursor<Vec<u8>>,
    bytes: usize,
}
impl Read for Counted {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        let count = self.inner.read(output)?;
        self.bytes += count;
        Ok(count)
    }
}
impl Seek for Counted {
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(from)
    }
}
#[test]
fn media_indexes_and_large_optional_tags_are_skipped_inside_descriptor_bounds() {
    let bytes = document(
        &[
            list(b"hdrl", &headers()),
            list(b"movi", &vec![9; 4 * 1024 * 1024]),
            chunk(b"idx1", &vec![8; 2 * 1024 * 1024]),
            list(
                b"INFO",
                &[
                    chunk(b"INAM", &vec![7; 128 * 1024]),
                    chunk(b"IART", b"After Media\0"),
                ]
                .concat(),
            ),
        ]
        .concat(),
    );
    let length = bytes.len();
    let mut wrapped = b"outside".to_vec();
    wrapped.extend(bytes);
    wrapped.extend(b"outside");
    let mut counted = Counted {
        inner: Cursor::new(wrapped),
        bytes: 0,
    };
    let mut range =
        crate::utils::range_reader::RangeReader::new(&mut counted, 7, length as u64).unwrap();
    let result = video(&mut range).unwrap();
    assert_eq!(result.artist, "After Media");
    assert_eq!((result.width, result.height), (640, 360));
    drop(range);
    assert!(counted.bytes < 4096, "read {} bytes", counted.bytes);
}
#[test]
fn duration_only_skips_text_and_preserves_unknown_and_measured_zero() {
    let bytes = document(
        &[
            list(b"hdrl", &headers()),
            list(b"INFO", &chunk(b"INAM", &vec![7; 64 * 1024])),
        ]
        .concat(),
    );
    let mut counted = Counted {
        inner: Cursor::new(bytes),
        bytes: 0,
    };
    assert_eq!(milliseconds(&mut counted), Some(100));
    assert!(counted.bytes < 512);
    for (scale, rate, micros, expected) in [(0, 0, 0, None), (1, 25, 0, Some(0))] {
        let bytes = document(&list(
            b"hdrl",
            &[main_header(0, micros), stream(b"vids", 0, 0, scale, rate)].concat(),
        ));
        assert_eq!(milliseconds(&mut Cursor::new(bytes)), expected);
    }
}
#[test]
fn malformed_sizes_padding_foreign_riff_and_excessive_chunks_are_rejected() {
    let bytes = document(&list(b"hdrl", &headers()));
    for end in 0..bytes.len() {
        assert!(video(&mut Cursor::new(&bytes[..end])).is_none(), "{end}");
    }
    let mut wave = bytes.clone();
    wave[8..12].copy_from_slice(b"WAVE");
    assert!(video(&mut Cursor::new(wave)).is_none());
    let missing_pad =
        document(&[list(b"hdrl", &headers()), b"JUNK\x01\x00\x00\x00x".to_vec()].concat());
    assert!(video(&mut Cursor::new(missing_pad)).is_none());
    let mut many = list(b"hdrl", &headers());
    for _ in 0..100_001 {
        many.extend(chunk(b"JUNK", &[]));
    }
    assert!(video(&mut Cursor::new(document(&many))).is_none());
}
#[test]
fn main_header_fallback_does_not_overflow_before_dividing_microseconds() {
    let bytes = document(&list(
        b"hdrl",
        &[main_header(u32::MAX, u32::MAX), stream(b"vids", 0, 0, 0, 0)].concat(),
    ));
    assert_eq!(
        milliseconds(&mut Cursor::new(bytes)),
        Some((u128::from(u32::MAX) * u128::from(u32::MAX) / 1000) as i64)
    );
}
#[test]
fn generated_mjpeg_avi_uses_shared_metadata_without_frame_decoding() {
    let bytes = include_bytes!("../../../../plain-rs/testdata/video-mjpeg.avi");
    let result = video(&mut Cursor::new(bytes)).unwrap();
    assert_eq!((result.width, result.height), (16, 16));
    assert_eq!(result.duration_ms, 120);
    assert_eq!(result.frame_rate, 25.0);
    assert_eq!(result.title, "AviFixture");
    assert_eq!(result.artist, "FixtureArtist");
}
