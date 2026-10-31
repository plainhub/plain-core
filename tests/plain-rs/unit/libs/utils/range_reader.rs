use super::*;
use std::io::Cursor;

#[test]
fn logical_offsets_and_eof_are_confined_to_the_resource_range() {
    let source = Cursor::new(b"outsideDATAsecret".to_vec());
    let mut reader = RangeReader::new(source, 7, 4).unwrap();
    assert_eq!(reader.seek(SeekFrom::End(0)).unwrap(), 4);
    let mut byte = [0];
    assert_eq!(reader.read(&mut byte).unwrap(), 0);
    assert_eq!(reader.seek(SeekFrom::Start(0)).unwrap(), 0);
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"DATA");
    assert_eq!(reader.seek(SeekFrom::End(-2)).unwrap(), 2);
    reader.read_exact(&mut byte).unwrap();
    assert_eq!(byte, [b'T']);
    for from in [SeekFrom::Start(5), SeekFrom::End(1), SeekFrom::Current(-4)] {
        assert!(reader.seek(from).is_err());
        assert_eq!(reader.stream_position().unwrap(), 3);
    }
    reader.read_exact(&mut byte).unwrap();
    assert_eq!(byte, [b'A']);
    assert_eq!(reader.read(&mut byte).unwrap(), 0);
}

#[test]
fn range_overflow_and_empty_ranges_are_handled_without_wrapping() {
    assert!(RangeReader::new(Cursor::new(Vec::<u8>::new()), u64::MAX, 1).is_err());
    let mut reader = RangeReader::new(Cursor::new(b"secret"), 2, 0).unwrap();
    assert_eq!(reader.read(&mut [0; 8]).unwrap(), 0);
    assert_eq!(reader.seek(SeekFrom::End(0)).unwrap(), 0);
    assert!(reader.seek(SeekFrom::Current(-1)).is_err());
}
