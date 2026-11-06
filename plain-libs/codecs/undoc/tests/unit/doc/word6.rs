use super::*;

/// A stream holding one Word 6.0 character page (page 1) for `runs` -- (start, end, grpprl)
/// -- and its index at offset 0.
fn chpx_stream(runs: &[(u32, u32, &[u8])]) -> (Vec<u8>, FcLcb) {
    let mut stream = vec![0u8; 1024];
    let page = &mut stream[512..1024];
    let crun = runs.len();
    for (i, &(start, end, _)) in runs.iter().enumerate() {
        page[i * 4..i * 4 + 4].copy_from_slice(&start.to_le_bytes());
        page[(i + 1) * 4..(i + 1) * 4 + 4].copy_from_slice(&end.to_le_bytes());
    }
    let mut at = 510;
    for (i, &(_, _, grpprl)) in runs.iter().enumerate() {
        if grpprl.is_empty() {
            continue;
        }
        at = (at - 1 - grpprl.len()) & !1;
        page[at] = grpprl.len() as u8;
        page[at + 1..at + 1 + grpprl.len()].copy_from_slice(grpprl);
        page[4 * (crun + 1) + i] = (at / 2) as u8;
    }
    page[511] = crun as u8;
    let mut index = Vec::new();
    index.extend_from_slice(&runs[0].0.to_le_bytes());
    index.extend_from_slice(&runs[crun - 1].1.to_le_bytes());
    index.extend_from_slice(&1u16.to_le_bytes());
    stream[..index.len()].copy_from_slice(&index);
    let plc = FcLcb {
        fc: 0,
        lcb: index.len() as u32,
    };
    (stream, plc)
}

#[test]
fn character_sprms_become_run_properties() {
    // Plain (a font only), bold and coloured, a superscript, "opposite of the style" bold
    // with an underline, and a run with no properties.
    let (stream, plc) = chpx_stream(&[
        (100, 110, &[93, 0, 0]),
        (110, 120, &[85, 1, 98, 6]),
        (120, 122, &[104, 1]),
        (122, 130, &[85, 0x81, 94, 1]),
        (130, 131, &[]),
    ]);
    let index = read_chpx(&stream, plc);
    assert_eq!(index.at(105), CharProps::default());
    assert!(index.at(115).bold);
    assert!(index.at(121).superscript);
    let toggled = index.at(125);
    assert!(toggled.bold && toggled.underline);
    assert_eq!(index.at(130), CharProps::default());
}

#[test]
fn an_unknown_sprm_stops_the_walk_and_keeps_what_was_read() {
    // Bold, then a code no Word 6.0 sprm has, then italic, which is not reached.
    let (stream, plc) = chpx_stream(&[(100, 110, &[85, 1, 255, 86, 1])]);
    let props = read_chpx(&stream, plc).at(100);
    assert!(props.bold && !props.italic);
}
