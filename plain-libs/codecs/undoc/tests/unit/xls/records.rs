use super::*;

#[test]
fn rk_numbers_decode_in_all_four_encodings() {
    assert_eq!(rk_value((42 << 2) | 0x02), 42.0);
    assert_eq!(rk_value(((-7i32 << 2) as u32) | 0x02), -7.0);
    assert_eq!(rk_value((1234 << 2) | 0x03), 12.34);
    let bits = (1.5f64.to_bits() >> 32) as u32;
    assert_eq!(rk_value(bits), 1.5);
}

#[test]
fn a_string_split_across_continue_changes_width_at_the_boundary() {
    // "ab" compressed, then a CONTINUE restarting with fHighByte for "한".
    let first: &[u8] = &[3, 0, 0x00, b'a', b'b'];
    let second: &[u8] = &[0x01, 0x5C, 0xD5];
    let segments = [first, second];
    let mut cursor = Cursor::new(&segments);
    assert_eq!(cursor.rich_extended_string().unwrap(), "ab한");
}

#[test]
fn records_carry_their_continuations() {
    let mut stream = Vec::new();
    for (kind, data) in [(SST, &b"xy"[..]), (CONTINUE, b"z"), (EOF, b"")] {
        stream.extend_from_slice(&kind.to_le_bytes());
        stream.extend_from_slice(&(data.len() as u16).to_le_bytes());
        stream.extend_from_slice(data);
    }
    let records: Vec<_> = Records::new(&stream, 0).map(|r| r.unwrap()).collect();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].segments, [&b"xy"[..], b"z"]);
    assert_eq!(records[1].kind, EOF);
}
