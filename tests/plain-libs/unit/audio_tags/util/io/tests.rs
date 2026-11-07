use super::*;
use std::io::{Cursor, SeekFrom};

#[test]
fn measuring_a_stream_preserves_its_read_position() {
    let mut source = Cursor::new(vec![1_u8, 2, 3, 4]);
    source.seek(SeekFrom::Start(2)).unwrap();
    assert_eq!(source.stream_len_hack().unwrap(), 4);
    assert_eq!(source.position(), 2);
    assert_eq!(Cursor::new(Vec::<u8>::new()).stream_len_hack().unwrap(), 0);
}

#[test]
fn extended_float_reads_reject_truncated_data() {
    let mut source = Cursor::new([0_u8; 9]);
    assert!(source.read_f80().is_err());
}
