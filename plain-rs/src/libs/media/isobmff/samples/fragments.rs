use super::super::{atoms, child, u32_at, u64_at};
use super::{defaults::Defaults, fragment_run::Run, sizes::Sizes};
use std::collections::{HashMap, HashSet};

pub(super) fn read(
    data: &[u8],
    moof: u64,
    selected: u32,
    defaults: &HashMap<u32, Defaults>,
    known: &HashSet<u32>,
    next_sample: &mut u32,
    next_time: &mut u64,
    output: &mut Vec<Run>,
) -> Option<()> {
    let mut implicit_base = moof;
    for traf in atoms(data)?.into_iter().filter(|a| &a.kind == b"traf") {
        let tfhd = child(traf.data, b"tfhd")?;
        let flags = u32_at(tfhd, 0)?;
        if flags & !0x03003b != 0 {
            return None;
        }
        let id = u32_at(tfhd, 4)?;
        if !known.contains(&id) {
            return None;
        }
        let mut default = defaults.get(&id).copied().unwrap_or(Defaults {
            description: 1,
            duration: 0,
            size: 0,
            flags: 0,
        });
        let mut position = 8;
        let base = if flags & 1 != 0 {
            let base = u64_at(tfhd, position)?;
            position += 8;
            base
        } else if flags & 0x020000 != 0 {
            moof
        } else {
            implicit_base
        };
        for (bit, field) in [
            (2, &mut default.description),
            (8, &mut default.duration),
            (16, &mut default.size),
            (32, &mut default.flags),
        ] {
            if flags & bit != 0 {
                *field = u32_at(tfhd, position)?;
                position += 4;
            }
        }
        if position != tfhd.len() {
            return None;
        }
        let mut time = if id == selected { *next_time } else { 0 };
        if let Some(tfdt) = child(traf.data, b"tfdt") {
            time = match u32_at(tfdt, 0)? {
                0 => u64::from(u32_at(tfdt, 4)?),
                0x01000000 => u64_at(tfdt, 4)?,
                _ => return None,
            };
        }
        let mut offset = base;
        for trun in atoms(traf.data)?.into_iter().filter(|a| &a.kind == b"trun") {
            let flags = u32_at(trun.data, 0)?;
            if flags & !0x01000f05 != 0 {
                return None;
            }
            let count = u32_at(trun.data, 4)?;
            if flags & 4 != 0 && flags & 0x400 != 0 {
                return None;
            }
            let mut position = 8;
            if flags & 1 != 0 {
                offset = base.checked_add_signed(i64::from(u32_at(trun.data, position)? as i32))?;
                position += 4;
            }
            let first_flags = if flags & 4 != 0 {
                let f = u32_at(trun.data, position)?;
                position += 4;
                Some(f)
            } else {
                None
            };
            let stride = (8..=11).filter(|bit| flags & (1 << bit) != 0).count() * 4;
            if usize::try_from(count).ok()?.checked_mul(stride)?
                != trun.data.len().checked_sub(position)?
            {
                return None;
            }
            let mut sizes = Vec::new();
            let mut durations = Vec::new();
            let mut sample_flags = Vec::new();
            if stride != 0 {
                for entry in trun.data[position..].chunks_exact(stride) {
                    let mut cursor = 0;
                    for (bit, values) in [
                        (0x100, &mut durations),
                        (0x200, &mut sizes),
                        (0x400, &mut sample_flags),
                    ] {
                        if flags & bit != 0 {
                            values.push(u32_at(entry, cursor)?);
                            cursor += 4;
                        }
                    }
                }
            }
            let run = Run {
                first: *next_sample,
                time,
                description: default.description,
                offset,
                sizes: Sizes {
                    count,
                    uniform: if flags & 0x200 != 0 { 0 } else { default.size },
                    entries: sizes,
                },
                durations,
                default_duration: default.duration,
                flags: sample_flags,
                default_flags: default.flags,
                first_flags,
            };
            let length = run.sizes.sum(0, count)?;
            offset = offset.checked_add(length)?;
            time = run.end_time()?;
            if id == selected {
                *next_sample = next_sample.checked_add(count)?;
                *next_time = time;
                output.push(run);
            }
        }
        implicit_base = offset;
    }
    Some(())
}
