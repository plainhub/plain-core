use super::*;
use std::io::{Cursor, ErrorKind};

#[test]
fn endian_and_signed_values_match_fixed_vectors() {
    assert_eq!(
        Cursor::new([1, 2, 3, 4]).read_u32::<BigEndian>().unwrap(),
        0x01020304
    );
    assert_eq!(
        Cursor::new([1, 2, 3, 4])
            .read_u32::<LittleEndian>()
            .unwrap(),
        0x04030201
    );
    assert_eq!(
        Cursor::new([0x12, 0x34, 0x56])
            .read_u24::<BigEndian>()
            .unwrap(),
        0x123456
    );
    assert_eq!(
        Cursor::new([0x12, 0x34, 0x56])
            .read_u24::<LittleEndian>()
            .unwrap(),
        0x563412
    );
    assert_eq!(
        Cursor::new([0x80, 0]).read_i16::<BigEndian>().unwrap(),
        i16::MIN
    );
    assert_eq!(
        Cursor::new([0xff; 4]).read_i32::<LittleEndian>().unwrap(),
        -1
    );
    assert_eq!(
        Cursor::new([0xff; 8]).read_u64::<LittleEndian>().unwrap(),
        u64::MAX
    );
    assert_eq!(
        Cursor::new([0xff; 16]).read_u128::<BigEndian>().unwrap(),
        u128::MAX
    );
}

#[test]
fn truncated_values_preserve_unexpected_eof() {
    assert_eq!(
        Cursor::new([1, 2, 3])
            .read_u32::<BigEndian>()
            .unwrap_err()
            .kind(),
        ErrorKind::UnexpectedEof
    );
    assert_eq!(
        Cursor::new([1, 2])
            .read_uint::<LittleEndian>(3)
            .unwrap_err()
            .kind(),
        ErrorKind::UnexpectedEof
    );
}

#[test]
fn variable_widths_decode_the_entire_requested_value() {
    assert_eq!(
        Cursor::new([0x01, 0x02, 0x03, 0x04, 0x05, 0x06])
            .read_uint::<BigEndian>(6)
            .unwrap(),
        0x010203040506
    );
    assert_eq!(
        Cursor::new([0x01, 0x02, 0x03, 0x04, 0x05, 0x06])
            .read_uint::<LittleEndian>(6)
            .unwrap(),
        0x060504030201
    );
}

#[test]
#[should_panic(expected = "integer width must be between one and eight bytes")]
fn zero_byte_integer_width_is_rejected() {
    let _ = Cursor::new([]).read_uint::<BigEndian>(0);
}

#[test]
fn slice_reads_support_both_byte_orders_and_ignore_trailing_bytes() {
    assert_eq!(BigEndian::read_u16(&[1, 2, 3]), 0x0102);
    assert_eq!(LittleEndian::read_u16(&[1, 2, 3]), 0x0201);
    assert_eq!(BigEndian::read_u32(&[1, 2, 3, 4, 5]), 0x01020304);
    assert_eq!(LittleEndian::read_u32(&[1, 2, 3, 4, 5]), 0x04030201);
    assert_eq!(
        BigEndian::read_u64(&[1, 2, 3, 4, 5, 6, 7, 8, 9]),
        0x0102030405060708
    );
    assert_eq!(
        LittleEndian::read_u64(&[1, 2, 3, 4, 5, 6, 7, 8, 9]),
        0x0807060504030201
    );
}

#[test]
fn fixed_width_reads_preserve_exact_values() {
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
