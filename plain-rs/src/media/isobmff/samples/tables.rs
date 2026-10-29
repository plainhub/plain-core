use super::super::{child, u32_at, u64_at};

pub(super) fn table(data: &[u8], stride: usize) -> Option<&[u8]> {
    if u32_at(data, 0)? != 0 {
        return None;
    }
    let count = usize::try_from(u32_at(data, 4)?).ok()?;
    let bytes = data.get(8..)?;
    (count.checked_mul(stride)? == bytes.len()).then_some(bytes)
}

use super::sizes::Sizes;

struct ChunkRun {
    first_chunk: u32,
    samples_per_chunk: u32,
    description: u32,
    first_sample: u32,
}

pub(super) struct Classic {
    pub sizes: Sizes,
    pub timing: Vec<(u64, u64)>,
    pub syncs: Option<Vec<u32>>,
    chunks: Vec<u64>,
    runs: Vec<ChunkRun>,
}

impl Classic {
    pub fn read(stbl: &[u8]) -> Option<Self> {
        let sizes = Sizes::read(child(stbl, b"stsz")?)?;
        let timing: Vec<_> = table(child(stbl, b"stts")?, 8)?
            .chunks_exact(8)
            .map(|e| {
                (
                    u64::from(u32_at(e, 0).unwrap()),
                    u64::from(u32_at(e, 4).unwrap()),
                )
            })
            .collect();
        if timing
            .iter()
            .try_fold(0u64, |sum, (count, _)| sum.checked_add(*count))?
            != u64::from(sizes.count)
        {
            return None;
        }
        let syncs = if let Some(data) = child(stbl, b"stss") {
            let entries: Vec<_> = table(data, 4)?
                .chunks_exact(4)
                .map(|e| u32_at(e, 0).unwrap())
                .collect();
            if entries.iter().any(|id| *id == 0 || *id > sizes.count)
                || entries.windows(2).any(|v| v[0] >= v[1])
            {
                return None;
            }
            Some(entries)
        } else {
            None
        };
        let chunks = if let Some(data) = child(stbl, b"stco") {
            table(data, 4)?
                .chunks_exact(4)
                .map(|e| u64::from(u32_at(e, 0).unwrap()))
                .collect::<Vec<_>>()
        } else if let Some(data) = child(stbl, b"co64") {
            table(data, 8)?
                .chunks_exact(8)
                .map(|e| u64_at(e, 0).unwrap())
                .collect()
        } else {
            return None;
        };
        let entries = table(child(stbl, b"stsc")?, 12)?;
        let mut runs: Vec<ChunkRun> = Vec::new();
        for entry in entries.chunks_exact(12) {
            let first_chunk = u32_at(entry, 0)?;
            let samples_per_chunk = u32_at(entry, 4)?;
            let description = u32_at(entry, 8)?;
            if first_chunk == 0
                || first_chunk as usize > chunks.len()
                || samples_per_chunk == 0
                || description == 0
            {
                return None;
            }
            let first_sample = if let Some(previous) = runs.last() {
                previous.first_sample.checked_add(
                    first_chunk
                        .checked_sub(previous.first_chunk)?
                        .checked_mul(previous.samples_per_chunk)?,
                )?
            } else {
                if first_chunk != 1 {
                    return None;
                }
                0
            };
            if runs
                .last()
                .is_some_and(|last| first_chunk <= last.first_chunk)
            {
                return None;
            }
            runs.push(ChunkRun {
                first_chunk,
                samples_per_chunk,
                description,
                first_sample,
            });
        }
        let described = if let Some(last) = runs.last() {
            last.first_sample.checked_add(
                (u32::try_from(chunks.len()).ok()? + 1 - last.first_chunk)
                    .checked_mul(last.samples_per_chunk)?,
            )?
        } else {
            if !chunks.is_empty() {
                return None;
            }
            0
        };
        if described != sizes.count {
            return None;
        }
        Some(Self {
            sizes,
            timing,
            syncs,
            chunks,
            runs,
        })
    }

    pub fn range(&self, index: u32, description: u32) -> Option<(u64, u32)> {
        let run = self
            .runs
            .iter()
            .rev()
            .find(|run| run.first_sample <= index)?;
        if run.description != description {
            return None;
        }
        let relative = index.checked_sub(run.first_sample)?;
        let chunk = run
            .first_chunk
            .checked_sub(1)?
            .checked_add(relative / run.samples_per_chunk)?;
        let within = relative % run.samples_per_chunk;
        let offset = self
            .chunks
            .get(chunk as usize)?
            .checked_add(self.sizes.sum(index - within, within)?)?;
        Some((offset, self.sizes.get(index)?))
    }
}
