use super::*;

#[test]
fn sprm_walk_reads_each_operand_size() {
    // sprmPFInTable (1 byte), sprmPItap (4 bytes), a variable one (size byte 3), sprmPFTtp.
    let grpprl = [
        0x16, 0x24, 0x01, //
        0x49, 0x66, 0x02, 0x00, 0x00, 0x00, //
        0x15, 0xC6, 0x03, 0xAA, 0xBB, 0xCC, //
        0x17, 0x24, 0x01,
    ];
    let mut props = ParaProps::default();
    apply_papx(&mut props, &grpprl);
    assert!(props.in_table);
    assert!(props.row_end);
    assert_eq!(props.depth, 2);
}

#[test]
fn a_truncated_operand_stops_the_walk_without_panicking() {
    let mut seen = Vec::new();
    for_each_sprm(&[0x16, 0x24, 0x01, 0x49, 0x66, 0x02], |sprm, _| {
        seen.push(sprm)
    });
    assert_eq!(seen, [SPRM_P_F_IN_TABLE]);
}

#[test]
fn lookup_finds_the_run_holding_an_offset() {
    let index = PapxIndex {
        runs: vec![
            (
                100,
                150,
                ParaProps {
                    istd: 1,
                    ..Default::default()
                },
            ),
            (
                150,
                300,
                ParaProps {
                    istd: 2,
                    ..Default::default()
                },
            ),
        ],
    };
    assert_eq!(index.at(100).map(|p| p.istd), Some(1));
    assert_eq!(index.at(149).map(|p| p.istd), Some(1));
    assert_eq!(index.at(150).map(|p| p.istd), Some(2));
    assert_eq!(index.at(300), None);
    assert_eq!(index.at(99), None);
}
