use super::{atoms, child, header, payload, rewrite, u32_at};
use std::{
    io::{self, Read, Seek, SeekFrom, Write},
    ops::Range,
    sync::atomic::AtomicBool,
};

fn metadata(data: &[u8]) -> Option<()> {
    let data = data.strip_prefix(&[0; 4]).unwrap_or(data);
    atoms(data)?
        .iter()
        .all(|a| matches!(&a.kind, b"hdlr" | b"keys" | b"ilst"))
        .then_some(())
}

pub(super) fn local_tracks(movie: &[u8]) -> Option<()> {
    for item in atoms(movie)? {
        match &item.kind {
            b"mvhd" | b"free" => {}
            b"meta" => metadata(item.data)?,
            b"udta" => {
                for entry in atoms(item.data)? {
                    if &entry.kind == b"meta" {
                        metadata(entry.data)?;
                    } else if entry.kind[0] != 0xa9 {
                        return None;
                    }
                }
            }
            b"trak" => {
                let mdia = child(item.data, b"mdia")?;
                let minf = child(mdia, b"minf")?;
                let dref = child(child(minf, b"dinf")?, b"dref")?;
                let refs = atoms(dref.get(8..)?)?;
                if u32_at(dref, 4)? != 1
                    || refs.len() != 1
                    || !matches!(&refs[0].kind, b"url " | b"alis")
                    || refs[0].data != [0, 0, 0, 1]
                {
                    return None;
                }
                let table = child(minf, b"stbl")?;
                let stsd = child(table, b"stsd")?;
                let entries = rewrite::sample_descriptions(stsd)?;
                if entries.is_empty() || entries.iter().any(|e| e.data.get(6..8) != Some(&[0, 1])) {
                    return None;
                }
                let mut offsets = 0;
                for entry in atoms(table)? {
                    match &entry.kind {
                        b"stco" | b"co64" => offsets += 1,
                        b"stsd" | b"stts" | b"ctts" | b"cslg" | b"stss" | b"stsc" | b"stsz"
                        | b"stz2" | b"sdtp" | b"sgpd" | b"sbgp" | b"stps" => {}
                        _ => return None,
                    }
                }
                if offsets != 1 {
                    return None;
                }
                if atoms(item.data)?
                    .iter()
                    .any(|a| !matches!(&a.kind, b"tkhd" | b"tapt" | b"edts" | b"mdia" | b"tref"))
                    || atoms(mdia)?
                        .iter()
                        .any(|a| !matches!(&a.kind, b"mdhd" | b"hdlr" | b"minf" | b"elng"))
                    || atoms(minf)?.iter().any(|a| {
                        !matches!(
                            &a.kind,
                            b"vmhd" | b"smhd" | b"nmhd" | b"hdlr" | b"dinf" | b"stbl"
                        )
                    })
                {
                    return None;
                }
            }
            _ => return None,
        }
    }
    Some(())
}

fn plan(reader: &mut (impl Read + Seek), length: u64) -> Option<(Range<u64>, Vec<u8>)> {
    let mut offset = 0;
    let mut media = None;
    while offset < length {
        let (kind, start, end) = header(reader, offset, length)?;
        match &kind {
            b"ftyp" | b"wide" | b"free" | b"skip" => {}
            b"mdat" if media.is_none() => media = Some((offset, start..end)),
            b"moov" if end == length => {
                let (insert, extent) = media?;
                let mut movie = payload(reader, offset, end)?;
                let header = usize::try_from(start - offset).ok()?;
                if u32_at(&movie, 0)? == 0 {
                    return None;
                }
                local_tracks(&movie[header..])?;
                super::rewrite::shift_chunks(
                    &mut movie[header..],
                    end - offset,
                    extent.start..=extent.end.checked_sub(1)?,
                )?;
                return Some((insert..offset, movie));
            }
            _ => return None,
        }
        offset = end;
    }
    None
}

pub(super) fn apply(
    reader: &mut (impl Read + Seek),
    writer: &mut impl Write,
    canceled: &AtomicBool,
) -> io::Result<bool> {
    let length = reader.seek(SeekFrom::End(0))?;
    let Some((media, movie)) = plan(reader, length) else {
        return Ok(false);
    };
    reader.seek(SeekFrom::Start(0))?;
    rewrite::copy(reader, writer, media.start, canceled)?;
    rewrite::check_canceled(canceled)?;
    writer.write_all(&movie)?;
    rewrite::copy(reader, writer, media.end - media.start, canceled)?;
    Ok(true)
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/media/isobmff/fast_start.rs"]
mod tests;
