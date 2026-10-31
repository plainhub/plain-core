use super::*;

fn packet(counter: u8, start: bool, payload: &[u8]) -> [u8; 188] {
    let mut bytes = [0xff; 188];
    bytes[..4].copy_from_slice(&[0x47, if start { 0x40 } else { 0 }, 0, 0x30 | counter]);
    let padding = 183 - payload.len();
    bytes[4] = padding as u8;
    if padding > 0 {
        bytes[5] = 0;
    }
    bytes[188 - payload.len()..].copy_from_slice(payload);
    bytes
}

fn pat() -> Vec<u8> {
    let mut bytes = vec![0, 0xb0, 13, 0, 1, 0xc1, 0, 0, 0, 1, 0xe1, 0];
    bytes.extend_from_slice(&crc(&bytes).to_be_bytes());
    bytes
}

#[test]
fn split_sections_require_continuity_and_ignore_identical_duplicates() {
    let section = pat();
    assert!(valid_section(&section, 0, 12));
    let first = packet(0, true, &[vec![0], section[..8].to_vec()].concat());
    let second = packet(1, false, &section[8..]);
    let mut sections = Sections::default();
    assert!(
        sections
            .packet(Packet::read(&first).unwrap(), &first)
            .is_empty()
    );
    assert!(
        sections
            .packet(Packet::read(&first).unwrap(), &first)
            .is_empty()
    );
    assert_eq!(
        sections.packet(Packet::read(&second).unwrap(), &second),
        vec![section.clone()]
    );
    let gap = packet(2, false, &section[8..]);
    let mut sections = Sections::default();
    sections.packet(Packet::read(&first).unwrap(), &first);
    assert!(
        sections
            .packet(Packet::read(&gap).unwrap(), &gap)
            .is_empty()
    );
}

#[test]
fn pointer_finishes_a_previous_section_before_reading_multiple_new_sections() {
    let section = pat();
    let first = packet(0, true, &[vec![0], section[..8].to_vec()].concat());
    let next = packet(
        1,
        true,
        &[
            vec![8],
            section[8..].to_vec(),
            section.clone(),
            section.clone(),
        ]
        .concat(),
    );
    let mut sections = Sections::default();
    sections.packet(Packet::read(&first).unwrap(), &first);
    assert_eq!(
        sections.packet(Packet::read(&next).unwrap(), &next),
        vec![section.clone(), section.clone(), section]
    );
}

#[test]
fn invalid_sections_cannot_bypass_crc_size_current_or_pointer_bounds() {
    let mut section = pat();
    section[8] ^= 1;
    assert!(!valid_section(&section, 0, 12));
    let mut section = pat();
    section[5] &= !1;
    let end = section.len() - 4;
    let checksum = crc(&section[..end]).to_be_bytes();
    section[end..].copy_from_slice(&checksum);
    assert!(!valid_section(&section, 0, 12));
    for payload in [&[0, 0, 0xb3, 0xff][..], &[100, 0][..]] {
        let bytes = packet(0, true, payload);
        let mut sections = Sections::default();
        assert!(
            sections
                .packet(Packet::read(&bytes).unwrap(), &bytes)
                .is_empty()
        );
        assert!(sections.bytes.is_empty());
    }
    let mut malformed = packet(0, true, &[0]);
    malformed[4] = 184;
    assert!(Packet::read(&malformed).is_none());
    malformed[4] = 0;
    malformed[1] |= 0x80;
    assert!(Packet::read(&malformed).is_none());
}

#[test]
fn duplicate_detection_ignores_only_updated_pcr_bytes() {
    let mut bytes = packet(3, false, &[1, 2, 3]);
    bytes[5] = 0x10;
    let mut continuity = Continuity::default();
    assert!(matches!(
        continuity.check(Packet::read(&bytes).unwrap(), &bytes),
        Order::Next
    ));
    bytes[6..12].copy_from_slice(&[1, 2, 3, 4, 5, 6]);
    assert!(matches!(
        continuity.check(Packet::read(&bytes).unwrap(), &bytes),
        Order::Duplicate
    ));
    bytes[187] ^= 1;
    assert!(matches!(
        continuity.check(Packet::read(&bytes).unwrap(), &bytes),
        Order::Gap
    ));
    bytes[4] = 6;
    assert!(Packet::read(&bytes).is_none());
}

#[test]
fn continuity_wraps_and_scrambled_or_reserved_payload_controls_are_rejected() {
    let mut continuity = Continuity::default();
    for counter in [14, 15, 0, 1] {
        let bytes = packet(counter, false, &[0]);
        assert!(matches!(
            continuity.check(Packet::read(&bytes).unwrap(), &bytes),
            Order::Next
        ));
    }
    for control in [0, 0x50, 0x90, 0xd0] {
        let mut bytes = packet(0, false, &[0]);
        bytes[3] = control;
        assert!(Packet::read(&bytes).is_none());
    }
}

#[test]
fn adaptation_lengths_match_the_packet_payload_control() {
    for control in [2, 3] {
        for length in 0..=255u8 {
            let mut bytes = [0; 188];
            bytes[0] = 0x47;
            bytes[3] = control << 4;
            bytes[4] = length;
            let parsed = Packet::read(&bytes);
            let valid = if control == 2 {
                length == 183
            } else {
                length <= 182
            };
            assert_eq!(
                parsed.is_some(),
                valid,
                "control={control}, length={length}"
            );
            if let Some(parsed) = parsed {
                assert_eq!(
                    parsed.payload.len(),
                    if control == 2 {
                        0
                    } else {
                        183 - usize::from(length)
                    }
                );
            }
        }
    }
    let mut bytes = [0; 188];
    bytes[0] = 0x47;
    bytes[3] = 0x10;
    assert_eq!(Packet::read(&bytes).unwrap().payload.len(), 184);
}
