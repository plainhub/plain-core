use super::records;

fn record(action: u32, name: &[u16], next: u32) -> Vec<u8> {
    let mut bytes = next.to_le_bytes().to_vec();
    bytes.extend(action.to_le_bytes());
    bytes.extend((name.len() as u32 * 2).to_le_bytes());
    bytes.extend(name.iter().copied().flat_map(u16::to_le_bytes));
    bytes
}

#[test]
fn byte_records_preserve_actions_utf16_and_padded_chaining() {
    let mut bytes = record(4, &[0xD800, 0x61], 16);
    bytes.extend(record(5, &[0x4E2D], 0));
    let entries = records(&bytes).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].action, 4);
    assert_eq!(entries[0].name, [0, 216, 97, 0]);
    assert_eq!(entries[1].action, 5);
    assert_eq!(entries[1].name, [45, 78]);
}

#[test]
fn malformed_lengths_and_offsets_are_rejected_before_events_are_exposed() {
    let valid = record(1, &[0x61], 0);
    for length in 0..valid.len() {
        assert!(records(&valid[..length]).is_err());
    }
    for length in [0, 1, u32::MAX] {
        let mut bytes = valid.clone();
        bytes[8..12].copy_from_slice(&length.to_le_bytes());
        assert!(records(&bytes).is_err());
    }
    for next in [4, 12, 14, 16, u32::MAX] {
        let bytes = record(1, &[0x61], next);
        assert!(records(&bytes).is_err());
    }
    let mut bytes = record(1, &[0x61], 16);
    bytes.extend([0; 2]);
    bytes.extend([0; 11]);
    assert!(records(&bytes).is_err());
}
