use super::{EXTENDED_CONTENT, objects, tags, u32_at};
use crate::media::cover::CoverData;
use std::io::{Read, Seek};

fn string(
    reader: &mut (impl Read + Seek),
    offset: &mut u64,
    end: u64,
    budget: &mut u64,
) -> Option<String> {
    objects::position(reader, *offset)?;
    let mut units = Vec::new();
    for _ in 0..32_768 {
        if offset.checked_add(2)? > end {
            return None;
        }
        *budget = budget.checked_sub(2)?;
        let mut bytes = [0; 2];
        reader.read_exact(&mut bytes).ok()?;
        *offset += 2;
        let unit = u16::from_le_bytes(bytes);
        if unit == 0 {
            return String::from_utf16(&units).ok();
        }
        units.push(unit);
    }
    None
}
fn picture(
    reader: &mut (impl Read + Seek),
    start: u64,
    end: u64,
    minimum: i8,
    budget: &mut u64,
) -> Option<(i8, CoverData)> {
    let header = tags::read(reader, start, 5, end, budget)?;
    if header[0] > 20 {
        return None;
    }
    let rank = match header[0] {
        3 => 2,
        0 => 1,
        _ => 0,
    };
    if rank <= minimum {
        return None;
    }
    let size = u64::from(u32_at(&header, 1)?);
    if size == 0 || size > 25 * 1024 * 1024 {
        return None;
    }
    let mut offset = start + 5;
    let mime = string(reader, &mut offset, end, budget)?;
    let _description = string(reader, &mut offset, end, budget)?;
    if offset.checked_add(size)? > end {
        return None;
    }
    *budget = budget.checked_sub(size)?;
    objects::position(reader, offset)?;
    let mut bytes = vec![0; usize::try_from(size).ok()?];
    reader.read_exact(&mut bytes).ok()?;
    Some((
        rank,
        CoverData {
            bytes,
            mime: if mime.is_empty() {
                "image/jpeg".into()
            } else {
                mime
            },
        },
    ))
}
pub(super) fn read(reader: &mut (impl Read + Seek)) -> Option<CoverData> {
    let mut selected: Option<(i8, CoverData)> = None;
    let mut metadata_budget = 4 * 1024 * 1024u64;
    let mut picture_budget = 2 * (25 * 1024 * 1024 + 128 * 1024 + 5u64);
    objects::visit(reader, true, |reader, object| {
        if object.guid == EXTENDED_CONTENT
            || matches!(object.guid, objects::METADATA | objects::METADATA_LIBRARY)
        {
            tags::descriptors(
                reader,
                object.data,
                object.end,
                object.guid != EXTENDED_CONTENT,
                &mut metadata_budget,
                |reader, record, _| {
                    if record.stream == 0
                        && record.kind == 1
                        && record.name.as_deref() == Some("WM/Picture")
                    {
                        if let Some(picture) = picture(
                            reader,
                            record.data,
                            record.end,
                            selected.as_ref().map_or(-1, |value| value.0),
                            &mut picture_budget,
                        ) {
                            selected = Some(picture);
                        }
                    }
                    Some(selected.as_ref().is_none_or(|value| value.0 != 2))
                },
            )?;
        }
        Some(selected.as_ref().is_none_or(|value| value.0 != 2))
    })?;
    selected.map(|value| value.1)
}
