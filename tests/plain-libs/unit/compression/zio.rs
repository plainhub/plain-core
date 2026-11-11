use super::*;
use crate::compression::{Compression, read::ZlibDecoder};
use std::io::Read;

#[derive(Default)]
struct ShortWriter {
    bytes: Vec<u8>,
    writes: Vec<(usize, usize, usize)>,
    attempts: usize,
    fail_on: Option<(usize, io::ErrorKind)>,
    flushes: usize,
    limit: Option<usize>,
}
impl Write for ShortWriter {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        self.attempts += 1;
        if let Some((at, kind)) = self.fail_on
            && self.attempts == at
        {
            self.fail_on = None;
            return if kind == io::ErrorKind::WriteZero {
                Ok(0)
            } else {
                Err(kind.into())
            };
        }
        let n = input.len().min(self.limit.unwrap_or(3));
        self.writes.push((input.as_ptr().addr(), input.len(), n));
        self.bytes.extend_from_slice(&input[..n]);
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        Ok(())
    }
}
fn assert_pending_bytes_never_move(sink: &ShortWriter) {
    let mut checked = 0;
    for pair in sink.writes.windows(2) {
        let (address, len, written) = pair[0];
        if len > written {
            assert_eq!(pair[1].0, address + written);
            assert_eq!(pair[1].1, len - written);
            checked += 1;
        }
    }
    assert!(checked > 8);
}
fn assert_payload(sink: &ShortWriter, input: &[u8]) {
    let mut output = Vec::new();
    ZlibDecoder::new(&sink.bytes[..])
        .read_to_end(&mut output)
        .unwrap();
    assert_eq!(output, input);
    assert_pending_bytes_never_move(sink);
}
#[test]
fn short_writes_borrow_consecutive_pending_bytes_without_sliding_the_buffer() {
    let input = b"independent compressed short-write payload ".repeat(128);
    let mut writer = Writer::new(
        ShortWriter::default(),
        Compress::new(Compression::default(), true),
    );
    writer.write_all(&input).unwrap();
    writer.finish().unwrap();
    let sink = writer.take_inner();
    assert_payload(&sink, &input);
    let mut reference =
        crate::compression::write::ZlibEncoder::new(Vec::new(), Compression::default());
    reference.write_all(&input).unwrap();
    assert_eq!(sink.bytes, reference.finish().unwrap());
}
#[test]
fn partial_output_survives_would_block_and_write_zero_without_duplicate_bytes() {
    let input = b"independent compressed retry payload ".repeat(128);
    for kind in [io::ErrorKind::WouldBlock, io::ErrorKind::WriteZero] {
        let mut writer = Writer::new(
            ShortWriter::default(),
            Compress::new(Compression::default(), true),
        );
        writer.write_all(&input).unwrap();
        let sink = writer.get_mut();
        sink.fail_on = Some((sink.attempts + 3, kind));
        assert_eq!(writer.finish().unwrap_err().kind(), kind);
        writer.finish().unwrap();
        let sink = writer.take_inner();
        assert_payload(&sink, &input);
    }
}
#[test]
fn flush_resumes_pending_output_before_encoding_more_bytes() {
    let input = b"independent flushed output payload ".repeat(128);
    let mut writer = Writer::new(
        ShortWriter::default(),
        Compress::new(Compression::default(), true),
    );
    writer.write_all(&input).unwrap();
    let sink = writer.get_mut();
    sink.fail_on = Some((sink.attempts + 3, io::ErrorKind::WouldBlock));
    assert_eq!(
        writer.flush().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    writer.flush().unwrap();
    assert_eq!(writer.get_ref().flushes, 1);
    writer.finish().unwrap();
    let sink = writer.take_inner();
    assert_payload(&sink, &input);
}

#[test]
fn ordinary_flush_keeps_pending_bytes_in_one_underlying_write() {
    let sink = ShortWriter {
        limit: Some(usize::MAX),
        ..Default::default()
    };
    let input = b"independent flush batching payload ".repeat(8);
    let mut writer = Writer::new(sink, Compress::new(Compression::default(), true));
    writer.write_all(&input).unwrap();
    writer.flush().unwrap();
    assert_eq!(writer.get_ref().writes.len(), 1);
    assert_eq!(writer.get_ref().flushes, 1);
    writer.finish().unwrap();
    let sink = writer.take_inner();
    let mut output = Vec::new();
    ZlibDecoder::new(sink.bytes.as_slice())
        .read_to_end(&mut output)
        .unwrap();
    assert_eq!(output, input);
}
