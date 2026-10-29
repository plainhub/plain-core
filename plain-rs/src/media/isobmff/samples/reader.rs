use super::super::{MAX_BOX_BYTES, atoms, child, header, payload, time_header, u32_at};
use super::{AvcTrack, avc, defaults, fragments, invalid, required, tables};
use std::{
    collections::HashSet,
    io::{self, Read, Seek, SeekFrom},
};

impl AvcTrack {
    pub fn read(reader: &mut (impl Read + Seek)) -> io::Result<Self> {
        let length = reader.seek(SeekFrom::End(0))?;
        let mut offset = 0;
        let mut movie = None;
        let mut moofs = Vec::new();
        let mut media = Vec::new();
        let mut budget = MAX_BOX_BYTES;
        let mut ftyp = false;
        while offset < length {
            let (kind, start, end) = required(header(reader, offset, length))?;
            match &kind {
                b"ftyp" => {
                    if end - start < 8 || (end - start) % 4 != 0 {
                        return Err(invalid());
                    }
                    ftyp = true;
                }
                b"moov" => {
                    if movie.replace((start, end)).is_some() {
                        return Err(invalid());
                    }
                }
                b"moof" => {
                    budget = required(budget.checked_sub(24))?;
                    moofs.push((offset, start, end));
                }
                b"mdat" => {
                    budget = required(budget.checked_sub(16))?;
                    media.push((start, end));
                }
                _ => {}
            }
            offset = end;
        }
        if !ftyp {
            return Err(invalid());
        }
        let (start, end) = required(movie)?;
        let movie = required(super::super::projection::sample_movie(reader, start, end))?;
        budget = required(budget.checked_sub(movie.len() as u64))?;
        let tracks = required(atoms(&movie))?;
        let mut known = HashSet::new();
        let mut selected = None;
        for track in tracks.iter().filter(|a| &a.kind == b"trak") {
            let tkhd = required(child(track.data, b"tkhd"))?;
            let id_offset = match tkhd.first() {
                Some(0) => 12,
                Some(1) => 20,
                _ => return Err(invalid()),
            };
            let id = required(u32_at(tkhd, id_offset))?;
            if id == 0 || !known.insert(id) {
                return Err(invalid());
            }
            let mdia = required(child(track.data, b"mdia"))?;
            if selected.is_none()
                && child(mdia, b"hdlr").and_then(|d| d.get(8..12)) == Some(b"vide")
            {
                selected = Some((id, mdia));
            }
        }
        let (id, mdia) =
            selected.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no video track"))?;
        let (timescale, duration_ticks) = required(child(mdia, b"mdhd").and_then(time_header))?;
        let stbl = required(child(mdia, b"minf").and_then(|d| child(d, b"stbl")))?;
        let config = required(child(stbl, b"stsd").and_then(avc::read))?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Unsupported,
                "non-AVC video track has no pure-Rust decoder",
            )
        })?;
        let classic = required(tables::Classic::read(stbl))?;
        let defaults = required(defaults::read(&movie))?;
        let mut next_sample = required(classic.sizes.count.checked_add(1))?;
        let mut next_time =
            required(classic.timing.iter().try_fold(0u64, |sum, (count, delta)| {
                sum.checked_add(count.checked_mul(*delta)?)
            }))?;
        let mut fragments = Vec::new();
        for (moof, start, end) in moofs {
            budget = required(budget.checked_sub(end - start))?;
            let bytes = required(payload(reader, start, end))?;
            required(fragments::read(
                &bytes,
                moof,
                id,
                &defaults,
                &known,
                &mut next_sample,
                &mut next_time,
                &mut fragments,
            ))?;
        }
        Ok(Self {
            width: config.width,
            height: config.height,
            timescale,
            duration_ticks: if duration_ticks == 0 {
                next_time
            } else {
                duration_ticks
            },
            nal_length_size: config.length_size,
            parameter_sets: config.parameter_sets,
            description: config.description,
            sample_count: next_sample - 1,
            classic,
            fragments,
            media,
        })
    }
}
