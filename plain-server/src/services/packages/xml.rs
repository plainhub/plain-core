use super::manifest::{Pool, u16_at, u32_at};
use anyhow::{Result, anyhow, ensure};
use std::collections::HashSet;

const OUTPUT_LIMIT: usize = 32 * 1024 * 1024;

fn name_start(value: char) -> bool {
    matches!(value, '_' | 'A'..='Z' | 'a'..='z')
        || matches!(value as u32,
        0xc0..=0xd6 | 0xd8..=0xf6 | 0xf8..=0x2ff | 0x370..=0x37d | 0x37f..=0x1fff |
        0x200c..=0x200d | 0x2070..=0x218f | 0x2c00..=0x2fef | 0x3001..=0xd7ff |
        0xf900..=0xfdcf | 0xfdf0..=0xfffd | 0x10000..=0xeffff)
}
fn name(value: &str) -> Result<()> {
    let mut chars = value.chars();
    ensure!(chars.next().is_some_and(name_start) && chars.all(|value| name_start(value) || matches!(value, '-' | '.' | '0'..='9' | '\u{b7}' | '\u{300}'..='\u{36f}' | '\u{203f}'..='\u{2040}')), "Invalid manifest XML name");
    Ok(())
}
fn push(output: &mut String, value: &str) -> Result<()> {
    ensure!(
        value.len() <= OUTPUT_LIMIT - output.len(),
        "Manifest XML output is too large"
    );
    output.push_str(value);
    Ok(())
}
fn escape(output: &mut String, value: &str) -> Result<()> {
    for value in value.chars() {
        match value {
            '&' => push(output, "&amp;")?,
            '<' => push(output, "&lt;")?,
            '>' => push(output, "&gt;")?,
            '"' => push(output, "&quot;")?,
            '\r' => push(output, "&#13;")?,
            '\n' => push(output, "&#10;")?,
            '\t' => push(output, "&#9;")?,
            value => {
                ensure!(
                    matches!(value as u32,0x20..=0xd7ff|0xe000..=0xfffd|0x10000..=0x10ffff),
                    "Invalid XML character"
                );
                push(output, value.encode_utf8(&mut [0; 4]))?;
            }
        }
    }
    Ok(())
}
fn complex(value: u32) -> f64 {
    let mantissa = (value as i32 >> 8) as f64;
    mantissa / [1.0, 128.0, 32768.0, 8388608.0][((value >> 4) & 3) as usize]
}
pub(super) fn typed(pool: &Pool<'_>, kind: u8, data: u32) -> Result<String> {
    Ok(match kind {
        0 => {
            if data == 1 {
                "@empty".into()
            } else {
                "@null".into()
            }
        }
        1 | 7 => format!("@0x{data:08x}"),
        2 | 8 => format!("?0x{data:08x}"),
        3 => pool.get(data)?,
        4 => {
            let value = f32::from_bits(data);
            ensure!(value.is_finite(), "Invalid manifest float");
            value.to_string()
        }
        5 => {
            let unit = ["px", "dp", "sp", "pt", "in", "mm"]
                .get((data & 0xf) as usize)
                .ok_or_else(|| anyhow!("Invalid dimension unit"))?;
            format!("{}{unit}", complex(data))
        }
        6 => {
            let unit = match data & 0xf {
                0 => "%",
                1 => "%p",
                _ => return Err(anyhow!("Invalid fraction unit")),
            };
            format!("{}{unit}", complex(data) * 100.0)
        }
        0x10 => (data as i32).to_string(),
        0x11 => format!("0x{data:x}"),
        0x12 => (data != 0).to_string(),
        0x1c..=0x1f => format!("#{data:08x}"),
        _ => return Err(anyhow!("Unsupported manifest value type {kind}")),
    })
}
fn qualified(
    pool: &Pool<'_>,
    namespace: u32,
    local: &str,
    namespaces: &[(String, String)],
    attribute: bool,
) -> Result<String> {
    name(local)?;
    if namespace == u32::MAX {
        return Ok(local.to_owned());
    }
    let uri = pool.get(namespace)?;
    if uri.is_empty() {
        return Ok(local.to_owned());
    }
    if uri == "http://www.w3.org/XML/1998/namespace" {
        return Ok(format!("xml:{local}"));
    }
    let (prefix, _) = namespaces
        .iter()
        .find(|(prefix, value)| *value == uri && (!attribute || !prefix.is_empty()))
        .ok_or_else(|| anyhow!("Unbound manifest namespace"))?;
    if prefix.is_empty() {
        Ok(local.to_owned())
    } else {
        Ok(format!("{prefix}:{local}"))
    }
}
fn bindings(namespaces: &[(String, String)]) -> Vec<(String, String)> {
    let mut seen = HashSet::new();
    let mut active: Vec<_> = namespaces
        .iter()
        .rev()
        .filter(|(prefix, _)| seen.insert(prefix))
        .cloned()
        .collect();
    for (_, uri) in namespaces {
        if uri.is_empty() {
            continue;
        }
        if active
            .iter()
            .any(|(prefix, value)| value == uri && !prefix.is_empty())
        {
            continue;
        }
        let prefix = (0..)
            .map(|index| format!("ns{index}"))
            .find(|prefix| !active.iter().any(|(bound, _)| bound == prefix))
            .unwrap();
        active.push((prefix, uri.clone()));
    }
    active
}
fn coerce(
    pool: &Pool<'_>,
    kind: u8,
    data: u32,
    resources: Option<&super::resources::Resources<'_>>,
) -> Result<String> {
    if matches!(kind, 1 | 7) {
        if let Some(style) = super::framework::style(data) {
            return Ok(format!("@android:style/{style}"));
        }
        if let Some(resources) = resources {
            if let Some(value) = resources.resolve(data)? {
                return Ok(value);
            }
        }
    }
    typed(pool, kind, data)
}
fn attribute_name(
    pool: &Pool<'_>,
    index: u32,
    map: Option<&[u8]>,
    resources: Option<&super::resources::Resources<'_>>,
) -> Result<String> {
    let name = pool.get(index)?;
    if !name.is_empty() {
        return Ok(name);
    }
    let map = map.ok_or_else(|| anyhow!("Missing manifest resource map"))?;
    let offset = (index as usize)
        .checked_mul(4)
        .ok_or_else(|| anyhow!("Manifest resource map offset overflow"))?;
    let id = u32_at(map, offset)?;
    if let Some(name) = super::framework::attribute(id) {
        return Ok(name.to_owned());
    }
    if let Some(resources) = resources {
        if let Some(symbol) = resources.name(id)? {
            return Ok(symbol.rsplit('/').next().unwrap_or(&symbol).to_owned());
        }
    }
    Err(anyhow!("Unknown manifest attribute resource ID 0x{id:08x}"))
}
pub(super) fn needs_resources(bytes: &[u8]) -> Result<bool> {
    ensure!(u16_at(bytes, 0)? == 3, "Binary Android manifest required");
    let mut at = u16_at(bytes, 2)? as usize;
    let size = u32_at(bytes, 4)? as usize;
    ensure!(
        at >= 8 && at <= size && size == bytes.len(),
        "Invalid manifest extent"
    );
    let mut pool = None;
    let mut resource_map = None;
    while at < size {
        let kind = u16_at(bytes, at)?;
        let header = u16_at(bytes, at + 2)? as usize;
        let length = u32_at(bytes, at + 4)? as usize;
        ensure!(
            header >= 8 && length >= header && length <= size - at,
            "Invalid manifest chunk"
        );
        let chunk = &bytes[at..at + length];
        match kind {
            1 => pool = Some(Pool::new(chunk, header)?),
            0x180 => {
                ensure!((length - header) % 4 == 0, "Invalid manifest resource map");
                resource_map = Some(&chunk[header..]);
            }
            0x102 => {
                ensure!(
                    header >= 16 && header + 20 <= length,
                    "Invalid manifest element"
                );
                let start = header + u16_at(chunk, header + 8)? as usize;
                let stride = u16_at(chunk, header + 10)? as usize;
                let count = u16_at(chunk, header + 12)? as usize;
                ensure!(
                    start >= header + 20
                        && stride >= 20
                        && start <= length
                        && count <= (length - start) / stride,
                    "Invalid manifest attributes"
                );
                let pool = pool
                    .as_ref()
                    .ok_or_else(|| anyhow!("Missing manifest string pool"))?;
                for index in 0..count {
                    let position = start + index * stride;
                    if u32_at(chunk, position + 8)? == u32::MAX
                        && matches!(chunk[position + 15], 1 | 7)
                    {
                        if super::framework::style(u32_at(chunk, position + 16)?).is_none() {
                            return Ok(true);
                        }
                    }
                    if pool.get(u32_at(chunk, position + 4)?)?.is_empty() {
                        let Some(map) = resource_map else {
                            return Ok(true);
                        };
                        let index = u32_at(chunk, position + 4)? as usize;
                        let offset = index
                            .checked_mul(4)
                            .ok_or_else(|| anyhow!("Manifest resource map offset overflow"))?;
                        if super::framework::attribute(u32_at(map, offset)?).is_none() {
                            return Ok(true);
                        }
                    }
                }
            }
            _ => {}
        }
        at += length;
    }
    Ok(false)
}
#[cfg(test)]
pub(super) fn render(bytes: &[u8]) -> Result<String> {
    render_with(bytes, None)
}
pub(super) fn render_with(
    bytes: &[u8],
    resources: Option<&super::resources::Resources<'_>>,
) -> Result<String> {
    ensure!(u16_at(bytes, 0)? == 3, "Binary Android manifest required");
    let mut offset = u16_at(bytes, 2)? as usize;
    let size = u32_at(bytes, 4)? as usize;
    ensure!(
        offset >= 8 && offset <= size && size == bytes.len(),
        "Invalid manifest extent"
    );
    let mut pool = None;
    let mut resource_map = None;
    let mut namespaces: Vec<(String, String)> = Vec::new();
    let mut stack = Vec::new();
    let mut root_seen = false;
    let mut output = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    while offset < size {
        let kind = u16_at(bytes, offset)?;
        let header = u16_at(bytes, offset + 2)? as usize;
        let length = u32_at(bytes, offset + 4)? as usize;
        ensure!(
            header >= 8 && length >= header && length <= size - offset,
            "Invalid manifest chunk"
        );
        let chunk = &bytes[offset..offset + length];
        if kind == 1 {
            ensure!(
                pool.is_none() && !root_seen,
                "Unexpected manifest string pool"
            );
            pool = Some(Pool::new(chunk, header)?);
        } else if kind == 0x180 {
            ensure!(
                resource_map.is_none() && !root_seen && (length - header) % 4 == 0,
                "Invalid manifest resource map"
            );
            resource_map = Some(&chunk[header..]);
        } else {
            let pool = pool
                .as_ref()
                .ok_or_else(|| anyhow!("Missing manifest string pool"))?;
            ensure!(header >= 16, "Invalid manifest node header");
            let active = bindings(&namespaces);
            match kind {
                0x100 | 0x101 => {
                    let prefix = u32_at(chunk, header)?;
                    let prefix = if prefix == u32::MAX {
                        String::new()
                    } else {
                        pool.get(prefix)?
                    };
                    if !prefix.is_empty() {
                        name(&prefix)?;
                    }
                    let uri = pool.get(u32_at(chunk, header + 4)?)?;
                    ensure!(
                        (!uri.is_empty() || prefix.is_empty())
                            && prefix != "xmlns"
                            && uri != "http://www.w3.org/2000/xmlns/"
                            && (prefix == "xml") == (uri == "http://www.w3.org/XML/1998/namespace"),
                        "Invalid namespace declaration"
                    );
                    if kind == 0x100 {
                        ensure!(namespaces.len() < 1024, "Manifest namespace limit exceeded");
                        namespaces.push((prefix, uri));
                    } else {
                        ensure!(
                            namespaces.pop() == Some((prefix, uri)),
                            "Unbalanced manifest namespace"
                        );
                    }
                }
                0x102 => {
                    ensure!(header + 20 <= length, "Truncated manifest element");
                    if stack.is_empty() {
                        ensure!(
                            !root_seen && pool.get(u32_at(chunk, header + 4)?)? == "manifest",
                            "Manifest root required"
                        );
                        root_seen = true;
                    }
                    let tag = qualified(
                        pool,
                        u32_at(chunk, header)?,
                        &pool.get(u32_at(chunk, header + 4)?)?,
                        &active,
                        false,
                    )?;
                    ensure!(stack.len() < 1024, "Manifest nesting limit exceeded");
                    for _ in 0..stack.len() {
                        push(&mut output, "\t")?;
                    }
                    push(&mut output, "<")?;
                    push(&mut output, &tag)?;
                    for (prefix, uri) in &active {
                        push(&mut output, " xmlns")?;
                        if !prefix.is_empty() {
                            push(&mut output, ":")?;
                            push(&mut output, prefix)?;
                        }
                        push(&mut output, "=\"")?;
                        escape(&mut output, uri)?;
                        push(&mut output, "\"")?;
                    }
                    let start = header + u16_at(chunk, header + 8)? as usize;
                    let stride = u16_at(chunk, header + 10)? as usize;
                    let count = u16_at(chunk, header + 12)? as usize;
                    ensure!(
                        start >= header + 20
                            && stride >= 20
                            && start <= length
                            && count <= (length - start) / stride,
                        "Invalid manifest attributes"
                    );
                    let mut attributes = HashSet::new();
                    for index in 0..count {
                        let position = start + index * stride;
                        let namespace = u32_at(chunk, position)?;
                        let local_index = u32_at(chunk, position + 4)?;
                        let local = attribute_name(pool, local_index, resource_map, resources)?;
                        ensure!(
                            namespace != u32::MAX || local != "xmlns",
                            "Namespace declarations must use namespace nodes"
                        );
                        let attribute = qualified(pool, namespace, &local, &active, true)?;
                        let uri = if namespace == u32::MAX {
                            String::new()
                        } else {
                            pool.get(namespace)?
                        };
                        ensure!(
                            attributes.insert((uri, local.clone())),
                            "Duplicate manifest attribute"
                        );
                        let raw = u32_at(chunk, position + 8)?;
                        ensure!(
                            u16_at(chunk, position + 12)? == 8 && chunk[position + 14] == 0,
                            "Invalid manifest typed value"
                        );
                        let value = if raw != u32::MAX {
                            pool.get(raw)?
                        } else {
                            coerce(
                                pool,
                                chunk[position + 15],
                                u32_at(chunk, position + 16)?,
                                resources,
                            )?
                        };
                        let value = if namespace != u32::MAX
                            && pool.get(namespace)? == "http://schemas.android.com/apk/res/android"
                        {
                            super::attributes::display(&local, value)
                        } else {
                            value
                        };
                        push(&mut output, " ")?;
                        push(&mut output, &attribute)?;
                        push(&mut output, "=\"")?;
                        escape(&mut output, &value)?;
                        push(&mut output, "\"")?;
                    }
                    push(&mut output, ">\n")?;
                    stack.push(tag);
                }
                0x103 => {
                    let tag = qualified(
                        pool,
                        u32_at(chunk, header)?,
                        &pool.get(u32_at(chunk, header + 4)?)?,
                        &active,
                        false,
                    )?;
                    ensure!(
                        stack.pop().as_ref() == Some(&tag),
                        "Unbalanced manifest element"
                    );
                    for _ in 0..stack.len() {
                        push(&mut output, "\t")?;
                    }
                    push(&mut output, "</")?;
                    push(&mut output, &tag)?;
                    push(&mut output, ">\n")?;
                }
                0x104 => {
                    ensure!(!stack.is_empty(), "Manifest text outside root");
                    ensure!(header + 12 <= length, "Invalid manifest text value");
                    let raw = u32_at(chunk, header)?;
                    let value = if raw == u32::MAX {
                        ensure!(
                            u16_at(chunk, header + 4)? == 8 && chunk[header + 6] == 0,
                            "Invalid manifest text value"
                        );
                        typed(pool, chunk[header + 7], u32_at(chunk, header + 8)?)?
                    } else {
                        pool.get(raw)?
                    };
                    escape(&mut output, &value)?;
                    push(&mut output, "\n")?;
                }
                _ => return Err(anyhow!("Unsupported manifest XML chunk {kind}")),
            }
        }
        offset += length;
    }
    ensure!(
        root_seen && stack.is_empty() && namespaces.is_empty(),
        "Incomplete manifest XML"
    );
    Ok(output)
}

#[cfg(test)]
#[path = "../../../tests/unit/services/packages/xml.rs"]
mod tests;
