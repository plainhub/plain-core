use super::*;
use std::io::{Cursor, ErrorKind};
#[test]
fn endian_values_and_writes_preserve_exact_bytes() {
    let bytes = [0x12, 0x34, 0x56, 0x78];
    assert_eq!(
        Cursor::new(bytes).read_u16::<LittleEndian>().unwrap(),
        0x3412
    );
    assert_eq!(Cursor::new(bytes).read_u16::<BigEndian>().unwrap(), 0x1234);
    assert_eq!(
        Cursor::new(bytes).read_u24::<LittleEndian>().unwrap(),
        0x563412
    );
    assert_eq!(
        Cursor::new(bytes).read_u24::<BigEndian>().unwrap(),
        0x123456
    );
    assert_eq!(
        Cursor::new(bytes).read_u32::<LittleEndian>().unwrap(),
        0x78563412
    );
    assert_eq!(
        Cursor::new(bytes).read_u32::<BigEndian>().unwrap(),
        0x12345678
    );
    let mut out = Vec::new();
    out.write_u16::<BigEndian>(0x1234).unwrap();
    out.write_u16::<LittleEndian>(0x5678).unwrap();
    assert_eq!(out, [0x12, 0x34, 0x78, 0x56]);
}
#[test]
fn short_inputs_remain_errors_for_each_width() {
    for size in 0..4 {
        let bytes = vec![0; size];
        assert_eq!(
            Cursor::new(&bytes)
                .read_u32::<LittleEndian>()
                .unwrap_err()
                .kind(),
            ErrorKind::UnexpectedEof
        );
        if size < 3 {
            assert!(Cursor::new(&bytes).read_u24::<BigEndian>().is_err());
        }
        if size < 2 {
            assert!(Cursor::new(&bytes).read_u16::<LittleEndian>().is_err());
        }
        if size == 0 {
            assert!(Cursor::new(&bytes).read_u8().is_err());
        }
    }
}
