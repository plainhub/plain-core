use super::*;

fn clx(pieces: &[(u32, u32, u32, bool)]) -> Vec<u8> {
    let mut plc = Vec::new();
    for (cp, ..) in pieces {
        plc.extend_from_slice(&cp.to_le_bytes());
    }
    plc.extend_from_slice(&pieces.last().unwrap().1.to_le_bytes());
    for &(_, _, fc, compressed) in pieces {
        plc.extend_from_slice(&0u16.to_le_bytes());
        let raw = if compressed {
            (fc * 2) | 0x4000_0000
        } else {
            fc
        };
        plc.extend_from_slice(&raw.to_le_bytes());
        plc.extend_from_slice(&0u16.to_le_bytes());
    }
    // A Prc in front, which the reader must step over.
    let mut out = vec![0x01, 0x02, 0x00, 0xAA, 0xBB, 0x02];
    out.extend_from_slice(&(plc.len() as u32).to_le_bytes());
    out.extend_from_slice(&plc);
    out
}

#[test]
fn mixed_pieces_read_in_character_order() {
    // "Hi" compressed at byte 100, then "é!" as UTF-16 at byte 200.
    let table = clx(&[(0, 2, 100, true), (2, 4, 200, false)]);
    let pieces = parse_clx(
        &table,
        FcLcb {
            fc: 0,
            lcb: table.len() as u32,
        },
    )
    .unwrap();
    let mut wd = vec![0u8; 300];
    wd[100..102].copy_from_slice(b"Hi");
    wd[200..204].copy_from_slice(&[0xE9, 0x00, b'!', 0x00]);

    let chars = read_chars(&wd, &pieces, 0, 4).unwrap();
    let text: String = chars.iter().map(|c| c.ch).collect();
    assert_eq!(text, "Hié!");
    assert_eq!(
        chars.iter().map(|c| c.fc).collect::<Vec<_>>(),
        [100, 101, 200, 202]
    );

    let tail: String = read_chars(&wd, &pieces, 1, 3)
        .unwrap()
        .iter()
        .map(|c| c.ch)
        .collect();
    assert_eq!(tail, "ié");
}

#[test]
fn a_piece_past_the_stream_is_an_error() {
    let table = clx(&[(0, 10, 100, true)]);
    let pieces = parse_clx(
        &table,
        FcLcb {
            fc: 0,
            lcb: table.len() as u32,
        },
    )
    .unwrap();
    let err = read_chars(&[0u8; 50], &pieces, 0, 10).unwrap_err();
    assert!(matches!(err, Error::InvalidData(_)), "{err}");
}

#[test]
fn a_clx_without_a_pcdt_is_an_error() {
    let err = parse_clx(&[0x05, 0, 0], FcLcb { fc: 0, lcb: 3 }).unwrap_err();
    assert!(matches!(err, Error::InvalidData(_)), "{err}");
}

#[test]
fn rejects_untrusted_text_counts_before_allocating() {
    assert!(read_chars(&[], &[], 0, u32::MAX).is_err());
}

#[test]
fn repeated_piece_ranges_cannot_expand_past_the_text_budget() {
    let piece = Piece {
        cp_start: 0,
        cp_end: 600_000,
        fc: 0,
        compressed: true,
    };
    assert!(read_chars(&vec![b'a'; 600_000], &[piece, piece], 0, 600_000).is_err());
}
