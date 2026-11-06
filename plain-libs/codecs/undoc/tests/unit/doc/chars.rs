use super::*;

#[test]
fn toggles_and_values_are_read() {
    let grpprl = [
        0x35, 0x08, 0x01, // bold on
        0x36, 0x08, 0x81, // italic: opposite of the (unresolved) style
        0x48, 0x2A, 0x01, // superscript
        0x3C, 0x08, 0x00, // not hidden
        0x09, 0x6A, 0x02, 0x00, 0xB1, 0x00, // symbol: U+00B1
    ];
    let mut props = CharProps::default();
    apply_chpx(&mut props, &grpprl);
    assert!(props.bold && props.italic && props.superscript);
    assert!(!props.hidden && !props.subscript);
    assert_eq!(props.symbol, Some((2, 0xB1)));
}

#[test]
fn an_unlisted_offset_is_plain_text() {
    let index = ChpxIndex {
        runs: vec![(
            10,
            20,
            CharProps {
                bold: true,
                ..Default::default()
            },
        )],
    };
    assert!(index.at(15).bold);
    assert_eq!(index.at(20), CharProps::default());
}
