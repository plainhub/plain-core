use super::{
    super::{atoms, child, time_header, track, u32_at},
    fragment_run::Run,
};
use std::collections::HashMap;

pub(super) fn atom(out: &mut Vec<u8>, kind: &[u8; 4], bytes: &[u8]) {
    out.extend(((bytes.len() + 8) as u32).to_be_bytes());
    out.extend(kind);
    out.extend(bytes);
}
fn words(out: &mut Vec<u8>, values: &[u32]) {
    for value in values {
        out.extend(value.to_be_bytes());
    }
}
fn table(out: &mut Vec<u8>, kind: &[u8; 4], version: u32, count: usize, entries: Vec<u8>) {
    let mut data = Vec::with_capacity(8 + entries.len());
    words(&mut data, &[version << 24, count as u32]);
    data.extend(entries);
    atom(out, kind, &data);
}
fn push_timing(rows: &mut Vec<(u32, u32)>, value: u32) {
    if let Some(last) = rows.last_mut().filter(|row| row.1 == value) {
        last.0 += 1;
    } else {
        rows.push((1, value));
    }
}
fn timing(out: &mut Vec<u8>, kind: &[u8; 4], version: u32, rows: &[(u32, u32)]) {
    let mut bytes = Vec::new();
    for &(count, value) in rows {
        words(&mut bytes, &[count, value]);
    }
    table(out, kind, version, rows.len(), bytes);
}

pub(super) struct Index {
    pub runs: Vec<Run>,
    pub count: u32,
    pub end: u64,
}
impl Index {
    fn tables(&self, source: &[u8]) -> Option<(Vec<u8>, u64)> {
        let mut out = Vec::new();
        let stsd = child(source, b"stsd")?;
        let descriptions = atoms(stsd.get(8..)?)?;
        if descriptions.len() != u32_at(stsd, 4)? as usize
            || descriptions.iter().any(|a| {
                !matches!(
                    &a.kind,
                    b"avc1" | b"avc3" | b"hvc1" | b"hev1" | b"mp4a" | b"av01" | b"vp09" | b"Opus"
                )
            })
        {
            return None;
        }
        if atoms(source)?.iter().any(|a| match &a.kind {
            b"stsd" => false,
            b"stts" | b"stsc" | b"stco" | b"co64" => a.data != [0; 8],
            b"stsz" => a.data != [0; 12],
            _ => true,
        }) {
            return None;
        }
        atom(&mut out, b"stsd", stsd);
        let mut durations = Vec::new();
        let mut composition = Vec::new();
        let mut sizes = Vec::new();
        words(&mut sizes, &[0, 0, self.count]);
        let mut sync = Vec::new();
        let mut dependencies = vec![0; 4];
        let mut chunks = Vec::new();
        let mut offsets = Vec::new();
        let mut previous = None;
        let mut clock = 0u64;
        let mut last_presentation = 0u64;
        let mut negative = false;
        let mut largest = 0i64;
        for (i, run) in self.runs.iter().enumerate() {
            if run.time != clock
                || run.description == 0
                || run.description as usize > descriptions.len()
                || run.sizes.count == 0
            {
                return None;
            }
            let shape = (run.sizes.count, run.description);
            if previous != Some(shape) {
                words(&mut chunks, &[i as u32 + 1, shape.0, shape.1]);
                previous = Some(shape);
            }
            offsets.extend(run.offset.to_be_bytes());
            for j in 0..run.sizes.count {
                let duration = run
                    .durations
                    .get(j as usize)
                    .copied()
                    .unwrap_or(run.default_duration);
                if duration == 0 {
                    return None;
                }
                let size = run.sizes.get(j)?;
                words(&mut sizes, &[size]);
                push_timing(&mut durations, duration);
                let cto = run.composition.get(j as usize).copied().unwrap_or(0);
                negative |= cto < 0;
                largest = largest.max(cto);
                push_timing(&mut composition, cto as u32);
                clock = clock.checked_add(duration.into())?;
                last_presentation = last_presentation.max(clock.checked_add_signed(cto)?);
                let flags = run.sample_flags(j);
                if flags & 0xf00effff != 0 {
                    return None;
                }
                dependencies.push((flags >> 20) as u8);
                if flags & 0x10000 == 0 {
                    words(&mut sync, &[run.first + j]);
                }
            }
        }
        if self.count == 0 || (negative && largest > i32::MAX.into()) {
            return None;
        }
        timing(&mut out, b"stts", 0, &durations);
        if composition.iter().any(|x| x.1 != 0) {
            timing(&mut out, b"ctts", negative as u32, &composition);
        }
        if dependencies[4..].iter().any(|bits| *bits != 0) {
            atom(&mut out, b"sdtp", &dependencies);
        }
        if sync.len() / 4 != self.count as usize {
            table(&mut out, b"stss", 0, sync.len() / 4, sync);
        }
        table(&mut out, b"stsc", 0, chunks.len() / 12, chunks);
        atom(&mut out, b"stsz", &sizes);
        table(&mut out, b"co64", 0, offsets.len() / 8, offsets);
        Some((out, last_presentation.max(clock)))
    }
}
fn duration(data: &[u8], pos: usize, ticks: u64) -> Option<Vec<u8>> {
    let mut data = data.to_vec();
    match data.first()? {
        0 => data
            .get_mut(pos..pos + 4)?
            .copy_from_slice(&u32::try_from(ticks).ok()?.to_be_bytes()),
        1 => data
            .get_mut(pos + 8..pos + 16)?
            .copy_from_slice(&ticks.to_be_bytes()),
        _ => return None,
    }
    Some(data)
}
fn replace(data: &[u8], changes: &[(&[u8; 4], Vec<u8>)]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    for a in atoms(data)? {
        let bytes = changes
            .iter()
            .find(|(k, _)| **k == a.kind)
            .map_or(a.data, |(_, v)| v.as_slice());
        atom(&mut out, &a.kind, bytes);
    }
    Some(out)
}
fn rebuild_track(data: &[u8], index: &Index, scale: u32) -> Option<(Vec<u8>, u64)> {
    let mdia = child(data, b"mdia")?;
    let minf = child(mdia, b"minf")?;
    let references = child(child(minf, b"dinf")?, b"dref")?;
    if u32_at(references, 4)? != 1 || child(references.get(8..)?, b"url ")? != [0, 0, 0, 1] {
        return None;
    }
    let (tables, end) = index.tables(child(minf, b"stbl")?)?;
    let media_header = child(mdia, b"mdhd")?;
    let (media_scale, _) = time_header(media_header)?;
    let mut start = 0;
    let edit = match child(data, b"edts") {
        Some(e) => Some(child(e, b"elst")?),
        None => None,
    };
    if let Some(e) = edit {
        if e.len() != 20 || u32_at(e, 0)? != 0 || u32_at(e, 4)? != 1 || u32_at(e, 16)? != 0x10000 {
            return None;
        }
        start = u64::from(u32_at(e, 12)?);
    }
    let mut ticks = end
        .checked_sub(start)?
        .checked_mul(scale.into())?
        .div_ceil(media_scale.into());
    let mut changes = Vec::new();
    if let Some(e) = edit {
        let declared = u32_at(e, 8)?;
        if declared != 0 {
            ticks = declared.into();
        }
        let mut entry = e.to_vec();
        entry[8..12].copy_from_slice(&u32::try_from(ticks).ok()?.to_be_bytes());
        changes.push((
            b"edts",
            replace(child(data, b"edts")?, &[(b"elst", entry)])?,
        ));
    }
    changes.push((b"tkhd", duration(child(data, b"tkhd")?, 20, ticks)?));
    changes.push((
        b"mdia",
        replace(
            mdia,
            &[
                (b"minf", replace(minf, &[(b"stbl", tables)])?),
                (b"mdhd", duration(media_header, 16, end)?),
            ],
        )?,
    ));
    Some((replace(data, &changes)?, ticks))
}

pub(super) fn movie(data: &[u8], indexes: &HashMap<u32, Index>) -> Option<Vec<u8>> {
    let mvhd = child(data, b"mvhd")?;
    let (scale, _) = time_header(mvhd)?;
    let mut out = Vec::new();
    let mut end = 0;
    for a in atoms(data)? {
        match &a.kind {
            b"trak" => {
                let (bytes, ticks) =
                    rebuild_track(a.data, indexes.get(&track(a.data)?.id)?, scale)?;
                end = end.max(ticks);
                atom(&mut out, b"trak", &bytes);
            }
            b"mvhd" | b"mvex" => {}
            b"udta" | b"free" | b"meta" => atom(&mut out, &a.kind, a.data),
            _ => return None,
        }
    }
    let mut result = Vec::new();
    atom(&mut result, b"mvhd", &duration(mvhd, 16, end)?);
    result.extend(out);
    Some(result)
}

pub(super) fn relocate_chunks(mut boxes: &mut [u8], media_start: u64) -> Option<()> {
    while !boxes.is_empty() {
        let length = u32_at(boxes, 0)? as usize;
        let (entry, rest) = boxes.split_at_mut_checked(length)?;
        let (header, body) = entry.split_at_mut_checked(8)?;
        match &header[4..] {
            b"trak" | b"mdia" | b"minf" | b"stbl" => relocate_chunks(body, media_start)?,
            b"co64" => {
                for chunk in body.get_mut(8..)?.chunks_exact_mut(8) {
                    let relative = u64::from_be_bytes(chunk.try_into().ok()?);
                    chunk.copy_from_slice(&relative.checked_add(media_start)?.to_be_bytes());
                }
            }
            _ => {}
        }
        boxes = rest;
    }
    Some(())
}
