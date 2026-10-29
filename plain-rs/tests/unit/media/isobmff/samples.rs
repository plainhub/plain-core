use super::*;
use std::io::Cursor;

fn atom(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut bytes = ((data.len() + 8) as u32).to_be_bytes().to_vec();
    bytes.extend(kind);
    bytes.extend(data);
    bytes
}
fn words(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_be_bytes()).collect()
}
fn table_box(kind: &[u8; 4], count: u32, values: &[u32]) -> Vec<u8> {
    atom(kind, &[words(&[0, count]), words(values)].concat())
}
fn description() -> Vec<u8> {
    let mut visual = vec![0; 78];
    visual[6..8].copy_from_slice(&1u16.to_be_bytes());
    visual[24..26].copy_from_slice(&320u16.to_be_bytes());
    visual[26..28].copy_from_slice(&240u16.to_be_bytes());
    visual.extend(atom(
        b"avcC",
        &[1, 66, 0, 30, 0xff, 0xe1, 0, 2, 0x67, 1, 1, 0, 2, 0x68, 2],
    ));
    atom(b"stsd", &[words(&[0, 1]), atom(b"avc1", &visual)].concat())
}
fn track(stbl: &[u8], ticks: u32) -> Vec<u8> {
    let mut tkhd = vec![0; 12];
    tkhd.extend(1u32.to_be_bytes());
    let mut mdhd = vec![0; 12];
    mdhd.extend(words(&[1000, ticks]));
    let mut handler = vec![0; 8];
    handler.extend(b"vide");
    atom(
        b"trak",
        &[
            atom(b"tkhd", &tkhd),
            atom(
                b"mdia",
                &[
                    atom(b"mdhd", &mdhd),
                    atom(b"hdlr", &handler),
                    atom(b"minf", &atom(b"stbl", stbl)),
                ]
                .concat(),
            ),
        ]
        .concat(),
    )
}
fn ftyp() -> Vec<u8> {
    atom(b"ftyp", b"isom\0\0\0\0isom")
}
fn classic_file(uniform: bool, co64: bool) -> Vec<u8> {
    let prefix = ftyp();
    let data = atom(b"mdat", &[1, 2, 3, 4, 5, 6, 7, 8]);
    let start = prefix.len() as u32 + 8;
    let offsets = if co64 {
        atom(
            b"co64",
            &[
                words(&[0, 2]),
                u64::from(start).to_be_bytes().to_vec(),
                u64::from(start + 4).to_be_bytes().to_vec(),
            ]
            .concat(),
        )
    } else {
        table_box(b"stco", 2, &[start, start + 4])
    };
    let sizes = if uniform {
        words(&[0, 2, 4])
    } else {
        words(&[0, 0, 4, 2, 2, 2, 2])
    };
    let stbl = [
        description(),
        atom(b"stsz", &sizes),
        table_box(b"stsc", 1, &[1, 2, 1]),
        offsets,
        table_box(b"stts", 1, &[4, 1000]),
        table_box(b"stss", 2, &[1, 3]),
    ]
    .concat();
    [prefix, data, atom(b"moov", &track(&stbl, 4000))].concat()
}
fn empty_tables() -> Vec<u8> {
    [
        description(),
        atom(b"stsz", &words(&[0, 0, 0])),
        table_box(b"stsc", 0, &[]),
        table_box(b"stco", 0, &[]),
        table_box(b"stts", 0, &[]),
    ]
    .concat()
}
fn patch_word(bytes: &mut [u8], kind: &[u8; 4], body_offset: usize, value: u32) {
    let at = bytes.windows(4).position(|w| w == kind).unwrap() + 4 + body_offset;
    bytes[at..at + 4].copy_from_slice(&value.to_be_bytes());
}

#[path = "samples/classic.rs"]
mod classic;
#[path = "samples/fragments.rs"]
mod fragments;
#[path = "samples/preservation.rs"]
mod preservation;
#[path = "samples/sparse.rs"]
mod sparse;
