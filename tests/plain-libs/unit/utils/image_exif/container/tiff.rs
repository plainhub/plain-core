use super::*;

fn header(little: bool, offset: u32) -> Vec<u8> {
    let mut data = if little {
        b"II\x2a\0".to_vec()
    } else {
        b"MM\0\x2a".to_vec()
    };
    data.extend_from_slice(&if little {
        offset.to_le_bytes()
    } else {
        offset.to_be_bytes()
    });
    data
}

#[test]
fn structural_validation_preserves_byte_orders_and_unknown_types() {
    for little in [false, true] {
        let mut data = header(little, 8);
        data.extend_from_slice(&if little {
            1u16.to_le_bytes()
        } else {
            1u16.to_be_bytes()
        });
        for value in [0x010f_u16, 99] {
            data.extend_from_slice(&if little {
                value.to_le_bytes()
            } else {
                value.to_be_bytes()
            });
        }
        for value in [u32::MAX, u32::MAX, 0] {
            data.extend_from_slice(&if little {
                value.to_le_bytes()
            } else {
                value.to_be_bytes()
            });
        }
        validate(&data).unwrap();
        for end in 0..data.len() {
            assert!(validate(&data[..end]).is_err(), "truncated at {end}");
        }
    }
}

#[test]
fn cyclic_root_ifds_and_invalid_child_pointers_are_rejected() {
    let mut data = header(true, 8);
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&8u32.to_le_bytes());
    assert!(validate(&data).is_err());
    let mut data = header(true, 8);
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&0x8769u16.to_le_bytes());
    data.extend_from_slice(&4u16.to_le_bytes());
    data.extend_from_slice(&1u32.to_le_bytes());
    data.extend_from_slice(&u32::MAX.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    assert!(validate(&data).is_err());
}

#[test]
fn repeated_child_references_reuse_structural_validation() {
    let mut data = header(true, 8);
    data.extend_from_slice(&2u16.to_le_bytes());
    for _ in 0..2 {
        data.extend_from_slice(&0x8769u16.to_le_bytes());
        data.extend_from_slice(&4u16.to_le_bytes());
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&38u32.to_le_bytes());
    }
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    let mut parser = Parser::default();
    parser.parse(&data).unwrap();
    assert_eq!(parser.ifd_reads, 2);
}

#[test]
fn overlapping_child_tables_have_a_bounded_validation_budget() {
    let mut data = header(true, 8);
    data.extend_from_slice(&16u16.to_le_bytes());
    for index in 0..16u32 {
        data.extend_from_slice(&0x8769u16.to_le_bytes());
        data.extend_from_slice(&4u16.to_le_bytes());
        data.extend_from_slice(&1u32.to_le_bytes());
        data.extend_from_slice(&(206 + index * 2).to_le_bytes());
    }
    data.extend_from_slice(&0u32.to_le_bytes());
    data.resize(434, 0);
    for index in 0..16 {
        data[206 + index * 2..208 + index * 2].copy_from_slice(&16u16.to_le_bytes());
    }
    let mut parser = Parser::default();
    let error = parser.parse(&data).unwrap_err();
    assert!(error.to_string().contains("work limit"));
    assert!(parser.ifd_reads <= 15);
}
