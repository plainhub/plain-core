use std::io::{self, Read, Seek, SeekFrom};

mod avc;
mod defaults;
mod fragment_run;
mod fragments;
mod reader;
mod sizes;
mod tables;
mod timing;

pub struct AvcTrack {
    pub width: u32,
    pub height: u32,
    pub timescale: u32,
    pub duration_ticks: u64,
    pub nal_length_size: usize,
    pub parameter_sets: Vec<Vec<u8>>,
    pub sample_count: u32,
    description: u32,
    classic: tables::Classic,
    fragments: Vec<fragment_run::Run>,
    media: Vec<(u64, u64)>,
}

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Invalid or oversized MP4 sample index",
    )
}
fn required<T>(value: Option<T>) -> io::Result<T> {
    value.ok_or_else(invalid)
}

impl AvcTrack {
    pub fn keyframe_at(&self, tick: u64) -> u32 {
        let mut previous = None;
        let mut first = None;
        if self.classic.sizes.count > 0 {
            let candidate = timing::locate_keyframe_sample(
                &self.classic.timing,
                self.classic.syncs.as_deref(),
                tick,
            )
            .clamp(1, self.classic.sizes.count);
            previous = Some(candidate);
            first = Some(candidate);
        }
        for run in &self.fragments {
            if first.is_none() {
                first = run.first_sync().and_then(|s| run.first.checked_add(s));
            }
            if run.time <= tick {
                if let Some(sync) = run.sync_before(run.candidate(tick)) {
                    previous = run.first.checked_add(sync);
                }
            }
        }
        previous
            .or(first)
            .unwrap_or(1)
            .clamp(1, self.sample_count.max(1))
    }

    pub fn read_sample(
        &self,
        reader: &mut (impl Read + Seek),
        id: u32,
        max_bytes: usize,
    ) -> io::Result<Option<Vec<u8>>> {
        if id == 0 || id > self.sample_count {
            return Ok(None);
        }
        let (offset, size) = if id <= self.classic.sizes.count {
            required(self.classic.range(id - 1, self.description))?
        } else {
            let run = required(
                self.fragments
                    .iter()
                    .find(|r| id >= r.first && id - r.first < r.sizes.count),
            )?;
            if run.description != self.description {
                return Err(invalid());
            }
            let index = id - run.first;
            (
                required(run.offset.checked_add(required(run.sizes.sum(0, index))?))?,
                required(run.sizes.get(index))?,
            )
        };
        let end = required(offset.checked_add(u64::from(size)))?;
        if !self
            .media
            .iter()
            .any(|(start, limit)| offset >= *start && end <= *limit)
        {
            return Err(invalid());
        }
        let size = usize::try_from(size).map_err(|_| invalid())?;
        if size > max_bytes {
            return Ok(None);
        }
        reader.seek(SeekFrom::Start(offset))?;
        let mut bytes = vec![0; size];
        reader.read_exact(&mut bytes)?;
        Ok(Some(bytes))
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/media/isobmff/samples.rs"]
mod tests;
