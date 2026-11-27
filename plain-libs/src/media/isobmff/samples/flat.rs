use super::super::{MAX_BOX_BYTES, atoms, child, header, payload, rewrite, track, u32_at};
use super::{
    defaults,
    flat_tables::{Index, atom, movie, relocate_chunks},
    fragments,
};
use std::{
    collections::{HashMap, HashSet},
    io::{self, Read, Seek, SeekFrom, Write},
    sync::atomic::AtomicBool,
};

struct Plan {
    prefix: Vec<u8>,
    media: Vec<(u64, u64)>,
    length: u64,
}
fn plan(input: &mut (impl Read + Seek), length: u64, canceled: &AtomicBool) -> Option<Plan> {
    let mut offset = 0;
    let mut ftyp = None;
    let mut moov = None;
    let mut media = Vec::new();
    let mut fragments = Vec::new();
    let mut budget = MAX_BOX_BYTES;
    while offset < length {
        let (kind, start, end) = header(input, offset, length)?;
        match &kind {
            b"ftyp" if ftyp.is_none() => ftyp = Some(payload(input, start, end)?),
            b"moov" if moov.is_none() => moov = Some(payload(input, start, end)?),
            b"moof" => {
                budget = budget.checked_sub(end - start + 64)?;
                fragments.push((offset, start, end));
            }
            b"mdat" => {
                budget = budget.checked_sub(16)?;
                media.push((start, end));
            }
            b"mfra" | b"free" | b"skip" => {}
            _ => return None,
        }
        offset = end;
    }
    if fragments.len() < 2 || media.is_empty() {
        return None;
    }
    let moov = moov?;
    let defaults = defaults::read(&moov)?;
    let mut indexes = HashMap::new();
    let mut video = false;
    for a in atoms(&moov)?.iter().filter(|a| &a.kind == b"trak") {
        let t = track(a.data)?;
        video |= t.video;
        if indexes
            .insert(
                t.id,
                Index {
                    runs: Vec::new(),
                    count: 0,
                    end: 0,
                },
            )
            .is_some()
        {
            return None;
        }
    }
    if !video {
        return None;
    }
    let known: HashSet<_> = indexes.keys().copied().collect();
    let mut media_offsets = Vec::new();
    let mut size = 0u64;
    for &(start, end) in &media {
        media_offsets.push(size);
        size = size.checked_add(end - start)?;
    }
    for (offset, start, end) in fragments {
        rewrite::check_canceled(canceled).ok()?;
        let bytes = payload(input, start, end)?;
        for a in atoms(&bytes)? {
            if &a.kind == b"mfhd" {
                continue;
            }
            if &a.kind != b"traf" {
                return None;
            }
            for t in atoms(a.data)? {
                match &t.kind {
                    b"tfdt" | b"tfhd" => {}
                    b"trun" => {
                        budget = budget.checked_sub(u64::from(u32_at(t.data, 4)?) * 32)?;
                    }
                    _ => return None,
                }
            }
        }
        for (&id, index) in &mut indexes {
            let mut first = index.count.checked_add(1)?;
            let old = index.runs.len();
            fragments::read(
                &bytes,
                offset,
                id,
                &defaults,
                &known,
                &mut first,
                &mut index.end,
                &mut index.runs,
                true,
            )?;
            index.count = first - 1;
            for run in &mut index.runs[old..] {
                let m = media
                    .partition_point(|&(start, _)| start <= run.offset)
                    .checked_sub(1)?;
                let (start, end) = media[m];
                if run.offset.checked_add(run.sizes.sum(0, run.sizes.count)?)? > end {
                    return None;
                }
                run.offset = media_offsets[m] + (run.offset - start);
            }
        }
    }
    let mut prefix = Vec::new();
    atom(&mut prefix, b"ftyp", &ftyp?);
    let mut indexed = movie(&moov, &indexes)?;
    let media_start = (prefix.len() + indexed.len() + 8 + 16) as u64;
    relocate_chunks(&mut indexed, media_start)?;
    atom(&mut prefix, b"moov", &indexed);
    Some(Plan {
        prefix,
        media,
        length: size,
    })
}
pub fn flatten_fragments(
    input: &mut (impl Read + Seek),
    out: &mut impl Write,
    canceled: &AtomicBool,
) -> io::Result<bool> {
    let length = input.seek(SeekFrom::End(0))?;
    let plan = plan(input, length, canceled);
    rewrite::check_canceled(canceled)?;
    let Some(plan) = plan else {
        return Ok(false);
    };
    out.write_all(&plan.prefix)?;
    out.write_all(&1u32.to_be_bytes())?;
    out.write_all(b"mdat")?;
    out.write_all(&(plan.length + 16).to_be_bytes())?;
    for (start, end) in plan.media {
        input.seek(SeekFrom::Start(start))?;
        rewrite::copy(input, out, end - start, canceled)?;
    }
    Ok(true)
}
