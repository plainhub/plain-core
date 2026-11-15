use super::*;

#[test]
fn source_reader_preserves_range_and_cancellation() {
    use std::io::Write;
    let mut file = crate::temporary::tempfile().unwrap();
    file.write_all(b"prefix-value-suffix").unwrap();
    let (stop, receiver) = watch::channel(false);
    let mut reader = SourceReader {
        reader: super::super::range_reader::RangeReader::new(file, 7, 5).unwrap(),
        stop: receiver,
    };
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"value");
    assert_eq!(reader.seek(SeekFrom::Start(0)).unwrap(), 0);
    stop.send(true).unwrap();
    assert_eq!(
        reader.read(&mut [0]).unwrap_err().kind(),
        std::io::ErrorKind::ConnectionAborted
    );
    assert_eq!(
        reader.seek(SeekFrom::Start(0)).unwrap_err().kind(),
        std::io::ErrorKind::ConnectionAborted
    );
    assert_eq!(reader.reader.stream_position().unwrap(), 0);
}
