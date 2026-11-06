//! Inline pictures ([MS-DOC] 2.9.192 `PICFAndOfficeArtData`): a picture character points, by
//! `sprmCPicLocation`, at a `PICF` in the `Data` stream, followed by the shape that draws it
//! and the blip store entry holding its bytes ([MS-ODRAW] `OfficeArtFBSE`, `OfficeArtBlip`).

/// `mfpf.mm` of a picture whose file name follows the header.
const MM_SHAPE_FILE: u16 = 0x0066;

const RT_SP_CONTAINER: u16 = 0xF004;
const RT_FOPT: u16 = 0xF00B;
const RT_TERTIARY_FOPT: u16 = 0xF122;
const RT_FBSE: u16 = 0xF007;

/// `wzDescription`: the shape's alternative text.
const PROP_DESCRIPTION: u16 = 0x0381;

/// A picture's bytes as a file, with what describes it.
#[derive(Debug)]
pub(super) struct Picture {
    pub data: Vec<u8>,
    pub extension: &'static str,
    pub alt_text: Option<String>,
}

struct Record {
    instance: u16,
    kind: u16,
    start: usize,
    end: usize,
}

fn record(data: &[u8], at: usize, limit: usize) -> Option<Record> {
    let h = data.get(at..at + 8)?;
    let ver_inst = u16::from_le_bytes([h[0], h[1]]);
    let len = u32::from_le_bytes([h[4], h[5], h[6], h[7]]) as usize;
    let start = at + 8;
    let end = start.checked_add(len).filter(|&e| e <= limit)?;
    Some(Record {
        instance: ver_inst >> 4,
        kind: u16::from_le_bytes([h[2], h[3]]),
        start,
        end,
    })
}

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    d.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    d.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// The picture whose `PICF` is at `offset` in the `Data` stream. None for a picture this
/// reader cannot turn into a file — a compressed metafile, or a structure it cannot follow.
pub(super) fn read(data: &[u8], offset: u32) -> Option<Picture> {
    let at = offset as usize;
    let lcb = u32_at(data, at)? as usize;
    let header = u16_at(data, at + 4)? as usize;
    let mm = u16_at(data, at + 6)?;
    let limit = at.checked_add(lcb).filter(|&l| l <= data.len())?;
    let mut p = at + header;
    if mm == MM_SHAPE_FILE {
        p += 1 + *data.get(p)? as usize;
    }

    let mut alt_text = None;
    let mut picture = None;
    while let Some(r) = record(data, p, limit) {
        match r.kind {
            RT_SP_CONTAINER => alt_text = alt_text.or_else(|| description(data, &r)),
            RT_FBSE => picture = picture.or_else(|| blip_in_fbse(data, &r)),
            _ => {}
        }
        p = r.end;
    }
    let (data, extension) = picture?;
    Some(Picture {
        data,
        extension,
        alt_text,
    })
}

/// The shape's alternative text, from its property tables.
fn description(data: &[u8], container: &Record) -> Option<String> {
    let mut p = container.start;
    while let Some(r) = record(data, p, container.end) {
        p = r.end;
        if r.kind != RT_FOPT && r.kind != RT_TERTIARY_FOPT {
            continue;
        }
        // `instance` properties of 6 bytes; complex values follow the table in order.
        let count = r.instance as usize;
        let mut complex = r.start + count * 6;
        for i in 0..count {
            let at = r.start + i * 6;
            let (id, value) = (u16_at(data, at)?, u32_at(data, at + 2)? as usize);
            let is_complex = id & 0x8000 != 0;
            if id & 0x3FFF == PROP_DESCRIPTION && is_complex {
                let units: Vec<u16> = data
                    .get(complex..(complex + value).min(r.end))?
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|&c| u16::from_le_bytes(c))
                    .take_while(|&u| u != 0)
                    .collect();
                let text = String::from_utf16_lossy(&units).trim().to_string();
                return (!text.is_empty()).then_some(text);
            }
            if is_complex {
                complex += value;
            }
        }
    }
    None
}

/// The blip a blip store entry holds, as file bytes and an extension.
fn blip_in_fbse(data: &[u8], fbse: &Record) -> Option<(Vec<u8>, &'static str)> {
    // 36 fixed bytes, then `cbName` bytes of name, then the embedded blip.
    let name_len = *data.get(fbse.start + 33)? as usize;
    let blip = record(data, fbse.start + 36 + name_len, fbse.end)?;
    // Bitmap blips: one or two 16-byte ids (the odd instance carries two), a tag byte, the file.
    let extension = match (blip.kind, blip.instance & !1) {
        (0xF01D | 0xF02A, 0x46A | 0x6E2) => "jpg",
        (0xF01E, 0x6E0) => "png",
        (0xF01F, 0x7A8) => "bmp",
        (0xF029, 0x6E4) => "tif",
        (0xF01A, 0x3D4) => return metafile(data, &blip, "emf"),
        (0xF01B, 0x216) => return metafile(data, &blip, "wmf"),
        // PICT (Macintosh) and anything else: not a file this reader produces.
        _ => return None,
    };
    let skip = 16 + if blip.instance & 1 == 1 { 16 } else { 0 } + 1;
    let bytes = data.get(blip.start + skip..blip.end)?;
    if extension == "bmp" {
        return Some((bmp_from_dib(bytes)?, extension));
    }
    Some((bytes.to_vec(), extension))
}

/// A metafile blip's file: one or two 16-byte ids, a 34-byte `OfficeArtMetafileHeader`, then
/// the metafile — DEFLATE-compressed (zlib framing) unless the header says it is stored.
fn metafile(
    data: &[u8],
    blip: &Record,
    extension: &'static str,
) -> Option<(Vec<u8>, &'static str)> {
    use std::io::Read;
    let header = blip.start + 16 + if blip.instance & 1 == 1 { 16 } else { 0 };
    let size = u32_at(data, header)? as usize;
    let saved = u32_at(data, header + 28)? as usize;
    let compression = *data.get(header + 32)?;
    let body = data.get(header + 34..(header + 34 + saved).min(blip.end))?;
    let bytes = match compression {
        0xFE => body.to_vec(),
        0x00 => {
            let mut out = Vec::with_capacity(size.min(64 << 20));
            flate2::read::ZlibDecoder::new(body)
                .take(64 << 20)
                .read_to_end(&mut out)
                .ok()?;
            out
        }
        _ => return None,
    };
    (!bytes.is_empty()).then_some((bytes, extension))
}

/// A BMP file from a device-independent bitmap: the 14-byte file header the DIB lacks.
fn bmp_from_dib(dib: &[u8]) -> Option<Vec<u8>> {
    let header_size = u32_at(dib, 0)? as usize;
    let bit_count = u16_at(dib, 14)?;
    let compression = u32_at(dib, 16)?;
    let colors_used = u32_at(dib, 32).unwrap_or(0) as usize;
    let palette = match (colors_used, bit_count) {
        (0, 1..=8) => 1usize << bit_count,
        (n, _) => n,
    };
    // BI_BITFIELDS with a plain info header keeps its three masks after it.
    let masks = if compression == 3 && header_size == 40 {
        12
    } else {
        0
    };
    let pixels_at = 14 + header_size + masks + palette * 4;
    let mut out = Vec::with_capacity(14 + dib.len());
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&((14 + dib.len()) as u32).to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(pixels_at as u32).to_le_bytes());
    out.extend_from_slice(dib);
    Some(out)
}

#[cfg(test)]
#[path = "../../tests/unit/doc/pictures.rs"]
pub(super) mod tests;
