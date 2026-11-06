//! The piece table ([MS-DOC] 2.9.38 `Clx`, 2.8.35 `PlcPcd`): where each run of document
//! characters sits in the `WordDocument` stream, and how it is encoded.

use super::fib::FcLcb;
use crate::error::{Error, Result};

/// One piece: character positions `[cp_start, cp_end)` stored from byte `fc` on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Piece {
    pub cp_start: u32,
    pub cp_end: u32,
    /// Byte offset of the first character in the `WordDocument` stream.
    pub fc: u32,
    /// One byte per character (cp1252) rather than UTF-16LE.
    pub compressed: bool,
}

/// A character of the document together with the stream offset it was read from — the key
/// paragraph and character properties are looked up by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DocChar {
    pub ch: char,
    pub fc: u32,
    /// Character position in the document's text.
    pub cp: u32,
}

fn invalid(what: impl Into<String>) -> Error {
    Error::InvalidData(format!("Word document: {}", what.into()))
}

/// Read the piece table out of the `Clx` in the table stream.
pub(super) fn parse_clx(table: &[u8], clx: FcLcb) -> Result<Vec<Piece>> {
    let start = clx.fc as usize;
    let end = start
        .checked_add(clx.lcb as usize)
        .filter(|&end| end <= table.len())
        .ok_or_else(|| invalid("the piece table lies outside the table stream"))?;
    let clx = &table[start..end];

    // Zero or more Prc (0x01, i16 size, grpprl), then exactly one Pcdt (0x02).
    let mut at = 0;
    while clx.get(at) == Some(&0x01) {
        let size = clx
            .get(at + 1..at + 3)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .filter(|&n| n >= 0)
            .ok_or_else(|| invalid("a property modifier in the piece table is truncated"))?;
        at += 3 + size as usize;
    }
    if clx.get(at) != Some(&0x02) {
        return Err(invalid(
            "the piece table has no piece descriptor table (Pcdt)",
        ));
    }
    let lcb = clx
        .get(at + 1..at + 5)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
        .ok_or_else(|| invalid("the piece descriptor table is truncated"))?;
    let plc = clx
        .get(at + 5..at + 5 + lcb)
        .ok_or_else(|| invalid("the piece descriptor table is truncated"))?;

    // PlcPcd: (n + 1) character positions, then n 8-byte piece descriptors.
    if plc.len() < 4 || (plc.len() - 4) % 12 != 0 {
        return Err(invalid("the piece descriptor table has an impossible size"));
    }
    let n = (plc.len() - 4) / 12;
    let cp = |i: usize| u32::from_le_bytes(plc[i * 4..i * 4 + 4].try_into().unwrap());
    let mut pieces = Vec::with_capacity(n);
    for i in 0..n {
        let pcd = 4 * (n + 1) + i * 8;
        let fc_compressed = u32::from_le_bytes(plc[pcd + 2..pcd + 6].try_into().unwrap());
        let compressed = fc_compressed & 0x4000_0000 != 0;
        let fc = fc_compressed & 0x3FFF_FFFF;
        let (cp_start, cp_end) = (cp(i), cp(i + 1));
        if cp_end < cp_start {
            return Err(invalid(
                "the piece table's character positions run backwards",
            ));
        }
        pieces.push(Piece {
            cp_start,
            cp_end,
            // A compressed piece stores its offset doubled.
            fc: if compressed { fc / 2 } else { fc },
            compressed,
        });
    }
    Ok(pieces)
}

/// The characters at positions `[from, to)`, in order, each with its stream offset.
///
/// Positions no piece covers are absent from the result rather than invented; a piece that
/// points past the end of the stream is an error, since its text cannot be recovered.
pub(super) fn read_chars(
    word_document: &[u8],
    pieces: &[Piece],
    from: u32,
    to: u32,
) -> Result<Vec<DocChar>> {
    if to.saturating_sub(from) > 1_000_000 {
        return Err(invalid("document text character limit exceeded"));
    }
    let mut out = Vec::new();
    for piece in pieces {
        let start = piece.cp_start.max(from);
        let end = piece.cp_end.min(to);
        if start >= end {
            continue;
        }
        if out.len().saturating_add((end - start) as usize) > 1_000_000 {
            return Err(invalid("document text character limit exceeded"));
        }
        let width = if piece.compressed { 1 } else { 2 };
        let first = piece.fc as usize + (start - piece.cp_start) as usize * width;
        let len = (end - start) as usize * width;
        let bytes = word_document.get(first..first + len).ok_or_else(|| {
            invalid("a piece of text lies past the end of the WordDocument stream")
        })?;
        if piece.compressed {
            for (i, &b) in bytes.iter().enumerate() {
                out.push(DocChar {
                    // Compressed text is Windows-1252 ([MS-DOC] 2.4.1).
                    ch: crate::codepage::windows_1252(b),
                    fc: (first + i) as u32,
                    cp: start + i as u32,
                });
            }
        } else {
            let units: Vec<u16> = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&c| u16::from_le_bytes(c))
                .collect();
            let mut i = 0;
            for decoded in char::decode_utf16(units.iter().copied()) {
                let unit_len = match decoded {
                    Ok(c) if (c as u32) > 0xFFFF => 2,
                    _ => 1,
                };
                out.push(DocChar {
                    ch: decoded.unwrap_or('\u{FFFD}'),
                    fc: (first + i * 2) as u32,
                    cp: start + i as u32,
                });
                i += unit_len;
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
#[path = "../../tests/unit/doc/text.rs"]
mod tests;
