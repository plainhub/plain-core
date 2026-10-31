use super::{atoms, child, u32_at};
use std::collections::HashMap;

pub(super) fn read(movie: &[u8]) -> HashMap<&'static str, String> {
    let mut fields = HashMap::new();
    collect(movie, &mut fields, 0);
    fields
}

fn key(name: &str) -> Option<&'static str> {
    if name.eq_ignore_ascii_case("com.android.capture.fps") {
        return Some("frameRate");
    }
    match name {
        "com.apple.quicktime.title" => Some("title"),
        "com.apple.quicktime.artist" => Some("artist"),
        "com.apple.quicktime.album" => Some("album"),
        "com.apple.quicktime.genre" => Some("genre"),
        "com.apple.quicktime.author" | "com.apple.quicktime.writer" => Some("writer"),
        "com.apple.quicktime.composer" => Some("composer"),
        "com.apple.quicktime.creationdate" => Some("date"),
        _ => None,
    }
}
fn standard(kind: &[u8; 4]) -> Option<&'static str> {
    match kind {
        b"\xa9nam" | b"titl" => Some("title"),
        b"\xa9ART" | b"perf" => Some("artist"),
        b"\xa9alb" | b"albm" => Some("album"),
        b"\xa9gen" | b"gnre" => Some("genre"),
        b"\xa9wrt" => Some("composer"),
        b"auth" => Some("writer"),
        b"\xa9day" => Some("date"),
        _ => None,
    }
}
fn text(data: &[u8], encoding: u32) -> Option<String> {
    if data.len() > 64 * 1024 {
        return None;
    }
    let text = match encoding {
        0 | 1 => std::str::from_utf8(data).ok()?.to_owned(),
        2 if data.len() % 2 == 0 => String::from_utf16(
            &data
                .chunks_exact(2)
                .map(|part| u16::from_be_bytes([part[0], part[1]]))
                .collect::<Vec<_>>(),
        )
        .ok()?,
        23 if data.len() == 4 => {
            let value = f32::from_bits(u32::from_be_bytes(data.try_into().ok()?));
            if !value.is_finite() {
                return None;
            }
            value.to_string()
        }
        _ => return None,
    };
    let text = text.trim_end_matches('\0');
    (!text.is_empty()).then(|| text.to_owned())
}
fn collect(data: &[u8], fields: &mut HashMap<&'static str, String>, depth: u8) {
    if depth > 2 {
        return;
    }
    let Some(children) = atoms(data) else { return };
    for item in children {
        if &item.kind == b"udta" {
            collect(item.data, fields, depth + 1);
        }
        if &item.kind == b"meta" {
            let data = if item.data.starts_with(&[0; 4]) {
                &item.data[4..]
            } else {
                item.data
            };
            let keys = child(data, b"keys").and_then(|data| {
                let count = usize::try_from(u32_at(data, 4)?).ok()?;
                let keys = atoms(data.get(8..)?)?;
                (count == keys.len()).then_some(keys)
            });
            let Some(items) = child(data, b"ilst").and_then(atoms) else {
                continue;
            };
            for item in items {
                let name = standard(&item.kind).or_else(|| {
                    let index = usize::try_from(u32::from_be_bytes(item.kind))
                        .ok()?
                        .checked_sub(1)?;
                    let item = keys.as_ref()?.get(index)?;
                    if &item.kind != b"mdta" {
                        return None;
                    }
                    key(std::str::from_utf8(item.data).ok()?)
                });
                let Some(name) = name else { continue };
                let Some(value) = child(item.data, b"data").and_then(|data| {
                    if &item.kind == b"gnre" {
                        let code = u16::from_be_bytes(data.get(8..10)?.try_into().ok()?);
                        return lofty::id3::v1::GENRES
                            .get(usize::from(code).checked_sub(1)?)
                            .map(|name| name.to_string());
                    }
                    text(data.get(8..)?, u32_at(data, 0)?)
                }) else {
                    continue;
                };
                fields.entry(name).or_insert(value);
            }
        } else if let Some(name) = standard(&item.kind) {
            let value = if matches!(&item.kind, b"titl" | b"perf" | b"albm" | b"gnre" | b"auth") {
                item.data
                    .get(6..)
                    .and_then(|data| text(data.split(|byte| *byte == 0).next()?, 1))
            } else {
                item.data.get(4..).and_then(|data| text(data, 1))
            };
            if let Some(value) = value {
                fields.entry(name).or_insert(value);
            }
        }
    }
}
