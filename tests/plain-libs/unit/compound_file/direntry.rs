use super::*;

#[test]
fn reads_a_full_length_utf16_directory_name_and_rejects_invalid_lengths() {
    let name = format!("{}a", "🙂".repeat(15));
    let mut bytes = [0; consts::DIR_ENTRY_LEN];
    for (index, unit) in name.encode_utf16().enumerate() {
        bytes[index * 2..][..2].copy_from_slice(&unit.to_le_bytes());
    }
    bytes[64..66].copy_from_slice(&64_u16.to_le_bytes());
    bytes[66] = 2;
    bytes[67] = 1;
    bytes[68..80].fill(0xff);
    let entry = DirEntry::read_from(&mut &bytes[..], Version::V4).unwrap();
    assert_eq!(entry.name, name);
    for length in [63_u16, 66] {
        bytes[64..66].copy_from_slice(&length.to_le_bytes());
        let error = DirEntry::read_from(&mut &bytes[..], Version::V4).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
}
