use super::*;
use std::io::{Cursor, Read, Write};

const PAYLOAD: &[u8] = include_bytes!("../../../plain-rs/testdata/extracted-streams/payload.bin");
const ZLIB: &[u8] = include_bytes!("../../../plain-rs/testdata/extracted-streams/payload.zlib");
const GZIP: &[u8] = include_bytes!("../../../plain-rs/testdata/extracted-streams/payload.gz");
const RAW: &[u8] = include_bytes!("../../../plain-rs/testdata/extracted-streams/payload.deflate");

struct ShortRead<'a>(&'a [u8]);
impl Read for ShortRead<'_> {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        let len = output.len().min(self.0.len()).min(3);
        output[..len].copy_from_slice(&self.0[..len]);
        self.0 = &self.0[len..];
        Ok(len)
    }
}

#[test]
fn independent_streams_decode_through_production_adapters() {
    let mut output = Vec::new();
    read::GzDecoder::new(ShortRead(GZIP))
        .read_to_end(&mut output)
        .unwrap();
    assert_eq!(output, PAYLOAD);
    output.clear();
    read::ZlibDecoder::new(ShortRead(ZLIB))
        .read_to_end(&mut output)
        .unwrap();
    assert_eq!(output, PAYLOAD);
    output.clear();
    bufread::DeflateDecoder::new(RAW)
        .read_to_end(&mut output)
        .unwrap();
    assert_eq!(output, PAYLOAD);
}

#[test]
fn checksum_damage_is_rejected_and_raw_reader_preserves_trailing_bytes() {
    let mut gzip = GZIP.to_vec();
    let at = gzip.len() - 8;
    gzip[at] ^= 1;
    assert!(
        read::GzDecoder::new(&gzip[..])
            .read_to_end(&mut Vec::new())
            .is_err()
    );
    let mut zlib = ZLIB.to_vec();
    *zlib.last_mut().unwrap() ^= 1;
    assert!(
        read::ZlibDecoder::new(&zlib[..])
            .read_to_end(&mut Vec::new())
            .is_err()
    );
    let mut with_tail = RAW.to_vec();
    with_tail.extend_from_slice(b"tail");
    let mut decoder = bufread::DeflateDecoder::new(&with_tail[..]);
    let mut bytes = Vec::new();
    decoder.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, PAYLOAD);
    assert_eq!(decoder.into_inner(), b"tail");
}

#[test]
fn existing_compression_levels_and_writer_reset_keep_valid_output() {
    for level in [
        Compression::none(),
        Compression::fast(),
        Compression::default(),
        Compression::best(),
    ] {
        let mut encoder = write::ZlibEncoder::new(Vec::new(), level);
        for part in PAYLOAD.chunks(73) {
            encoder.write_all(part).unwrap();
        }
        let bytes = encoder.finish().unwrap();
        let mut output = Vec::new();
        read::ZlibDecoder::new(&bytes[..])
            .read_to_end(&mut output)
            .unwrap();
        assert_eq!(output, PAYLOAD);
    }
    let mut encoder = write::DeflateEncoder::new(Cursor::new(Vec::new()), Compression::default());
    encoder.write_all(b"first").unwrap();
    let first = encoder.reset(Cursor::new(Vec::new())).unwrap().into_inner();
    encoder.write_all(b"second").unwrap();
    let second = encoder.finish().unwrap().into_inner();
    for (bytes, expected) in [(first, b"first".as_slice()), (second, b"second".as_slice())] {
        let mut output = Vec::new();
        bufread::DeflateDecoder::new(&bytes[..])
            .read_to_end(&mut output)
            .unwrap();
        assert_eq!(output, expected);
    }
}

#[test]
fn bounded_reads_continue_without_losing_stream_state() {
    let mut decoder = read::ZlibDecoder::new(ZLIB);
    let mut first = [0u8; 65];
    decoder.read_exact(&mut first).unwrap();
    assert_eq!(first, PAYLOAD[..65]);
    let mut rest = Vec::new();
    decoder.read_to_end(&mut rest).unwrap();
    assert_eq!(rest, PAYLOAD[65..]);
}

#[test]
fn gzip_size_mismatch_and_truncated_streams_are_rejected() {
    let mut gzip = GZIP.to_vec();
    let last = gzip.len() - 1;
    gzip[last] ^= 1;
    assert!(
        read::GzDecoder::new(&gzip[..])
            .read_to_end(&mut Vec::new())
            .is_err()
    );
    for cut in [0, 1, 7, 10, GZIP.len() - 1] {
        assert!(
            read::GzDecoder::new(&GZIP[..cut])
                .read_to_end(&mut Vec::new())
                .is_err()
        );
    }
}
