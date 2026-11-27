use super::*;
use std::io::Cursor;

const MOV: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../plain-rs/testdata/tail-index.mov"
));

#[test]
fn moved_index_points_at_the_same_media_and_already_indexed_files_are_skipped() {
    let mut output = Vec::new();
    assert!(apply(&mut Cursor::new(MOV), &mut output, &false.into()).unwrap());
    let before = atoms(MOV).unwrap();
    let after = atoms(&output).unwrap();
    assert_eq!(
        before.iter().find(|a| &a.kind == b"mdat").unwrap().data,
        after.iter().find(|a| &a.kind == b"mdat").unwrap().data
    );
    let tables = |movie| {
        atoms(movie)
            .unwrap()
            .into_iter()
            .filter(|a| &a.kind == b"trak")
            .map(|t| {
                let mdia = child(t.data, b"mdia").unwrap();
                let table = child(child(mdia, b"minf").unwrap(), b"stbl").unwrap();
                child(table, b"stco").unwrap()[8..]
                    .chunks_exact(4)
                    .map(|v| u32_at(v, 0).unwrap() as usize)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    let original = tables(child(MOV, b"moov").unwrap());
    let moved = tables(child(&output, b"moov").unwrap());
    assert_eq!(original.len(), 2);
    let delta = child(MOV, b"moov").unwrap().len() + 8;
    for (a, b) in original.iter().zip(moved) {
        assert!(!a.is_empty());
        assert_eq!(b, a.iter().map(|offset| offset + delta).collect::<Vec<_>>());
    }
    let mut unchanged = Vec::new();
    assert!(!apply(&mut Cursor::new(output), &mut unchanged, &false.into()).unwrap());
    assert!(unchanged.is_empty());
}

#[test]
fn external_data_or_invalid_chunk_offsets_do_not_publish_a_partial_movie() {
    for kind in [b"stco", b"dref"] {
        let mut input = MOV.to_vec();
        let tag = input.windows(4).position(|b| b == kind).unwrap();
        if kind == b"stco" {
            input[tag + 12..tag + 16].fill(0xff);
        } else {
            input[tag + 23] = 0;
        }
        let mut output = Vec::new();
        assert!(!apply(&mut Cursor::new(input), &mut output, &false.into()).unwrap());
        assert!(output.is_empty());
    }
}
