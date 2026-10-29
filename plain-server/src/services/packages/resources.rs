use super::{framework, input::Bytes, manifest::Pool, xml};
use anyhow::{Result, anyhow, ensure};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
};

struct Type<'a> {
    bytes: Bytes<'a>,
    header: usize,
    count: usize,
    start: usize,
    flags: u8,
    score: (i32, i32),
    type_name: Option<Bytes<'a>>,
    keys: Option<Bytes<'a>>,
    type_id: u8,
}
struct Value<'t, 'a> {
    kind: u8,
    data: u32,
    key: u32,
    complex: bool,
    table: &'t Type<'a>,
}
pub(super) struct Resources<'a> {
    strings: Pool<'a>,
    types: HashMap<u16, Vec<Type<'a>>>,
    resolved: RefCell<HashMap<u32, Option<String>>>,
}
fn chunk<'a>(bytes: Bytes<'a>, offset: usize) -> Result<(Bytes<'a>, u16, usize)> {
    ensure!(
        offset <= bytes.len() && bytes.len() - offset >= 8,
        "Truncated APK resource header"
    );
    let kind = bytes.u16(offset)?;
    let header = bytes.u16(offset + 2)? as usize;
    let size = bytes.u32(offset + 4)? as usize;
    ensure!(
        header >= 8 && size >= header && size <= bytes.len().saturating_sub(offset),
        "Invalid APK resource chunk"
    );
    Ok((bytes.slice(offset..offset + size)?, kind, header))
}
fn pool_at<'a>(bytes: Bytes<'a>, offset: usize) -> Result<Option<Bytes<'a>>> {
    if offset == 0 {
        return Ok(None);
    }
    let (bytes, kind, header) = chunk(bytes, offset)?;
    ensure!(kind == 1 && header >= 28, "Invalid resource name pool");
    Ok(Some(bytes))
}
impl<'a> Type<'a> {
    fn entry(&self, index: usize) -> Result<Option<Value<'_, 'a>>> {
        let offset = if self.flags & 1 != 0 {
            let mut low = 0;
            let mut high = self.count;
            while low < high {
                let middle = low + (high - low) / 2;
                let found = self.bytes.u16(self.header + middle * 4)? as usize;
                if found < index {
                    low = middle + 1
                } else {
                    high = middle
                }
            }
            if low == self.count || self.bytes.u16(self.header + low * 4)? as usize != index {
                return Ok(None);
            }
            usize::from(self.bytes.u16(self.header + low * 4 + 2)?) * 4
        } else {
            if index >= self.count {
                return Ok(None);
            }
            if self.flags & 2 != 0 {
                let offset = self.bytes.u16(self.header + index * 2)?;
                if offset == u16::MAX {
                    return Ok(None);
                }
                usize::from(offset) * 4
            } else {
                let offset = self.bytes.u32(self.header + index * 4)?;
                if offset == u32::MAX {
                    return Ok(None);
                }
                offset as usize
            }
        };
        let at = self
            .start
            .checked_add(offset)
            .ok_or_else(|| anyhow!("Resource entry offset overflow"))?;
        ensure!(
            at.checked_add(8).is_some_and(|end| end <= self.bytes.len()),
            "Resource entry outside chunk"
        );
        let flags = self.bytes.u16(at + 2)?;
        if flags & 8 != 0 {
            ensure!(flags & 1 == 0, "Complex compact resource entry");
            return Ok(Some(Value {
                kind: (flags >> 8) as u8,
                data: self.bytes.u32(at + 4)?,
                key: self.bytes.u16(at)? as u32,
                complex: false,
                table: self,
            }));
        }
        let size = self.bytes.u16(at)? as usize;
        ensure!(
            size >= 8 && size <= self.bytes.len().saturating_sub(at),
            "Invalid resource entry size"
        );
        let key = self.bytes.u32(at + 4)?;
        if flags & 1 != 0 {
            ensure!(size >= 16, "Invalid complex resource header");
            let count = self.bytes.u32(at + 12)? as usize;
            ensure!(
                count <= (self.bytes.len() - at - size) / 12,
                "Invalid complex resource extent"
            );
            return Ok(Some(Value {
                kind: 0,
                data: 0,
                key,
                complex: true,
                table: self,
            }));
        }
        let at = at + size;
        ensure!(
            self.bytes.u16(at)? == 8 && self.bytes.byte(at + 2)? == 0,
            "Invalid resource value"
        );
        Ok(Some(Value {
            kind: self.bytes.byte(at + 3)?,
            data: self.bytes.u32(at + 4)?,
            key,
            complex: false,
            table: self,
        }))
    }
    fn name(&self, key: u32) -> Result<Option<String>> {
        let (Some(types), Some(keys)) = (self.type_name, self.keys) else {
            return Ok(None);
        };
        let types = Pool::from_bytes(types, types.u16(2)? as usize)?;
        let keys = Pool::from_bytes(keys, keys.u16(2)? as usize)?;
        Ok(Some(format!(
            "@{}/{}",
            types.get(self.type_id as u32 - 1)?,
            keys.get(key)?
        )))
    }
}
fn score(bytes: Bytes<'_>, header: usize) -> Result<(i32, i32)> {
    let size = bytes.u32(20)? as usize;
    ensure!(
        size >= 4 && size <= header - 20,
        "Invalid resource configuration"
    );
    let mut locale = 1;
    if size >= 12 {
        let language = bytes.read(28, 2)?;
        let country = bytes.read(30, 2)?;
        locale = if language.as_ref() == b"en" {
            if country.as_ref() == b"US" {
                3
            } else if country.as_ref() == [0, 0] {
                2
            } else {
                0
            }
        } else if language.as_ref() == [0, 0] || country.as_ref() == [0, 0] {
            1
        } else {
            0
        };
    }
    let density = if size >= 16 { bytes.u16(34)? } else { 0 };
    Ok((
        locale,
        if matches!(density, 0xfffe | 0xffff) {
            -1
        } else {
            density as i32
        },
    ))
}
impl<'a> Resources<'a> {
    pub(super) fn parse(bytes: &'a [u8]) -> Result<Self> {
        Self::from_bytes(Bytes::Slice(bytes))
    }
    pub(super) fn from_bytes(bytes: Bytes<'a>) -> Result<Self> {
        let (table, kind, header) = chunk(bytes, 0)?;
        ensure!(
            kind == 2 && header >= 12 && table.len() == bytes.len(),
            "Invalid APK resource table"
        );
        let expected = bytes.u32(8)?;
        let mut packages = 0;
        let mut offset = header;
        let mut strings = None;
        let mut types: HashMap<u16, Vec<Type<'a>>> = HashMap::new();
        let mut type_count = 0;
        while offset < bytes.len() {
            let (part, kind, header) = chunk(bytes, offset)?;
            match kind {
                1 => {
                    ensure!(strings.is_none(), "Duplicate resource value pool");
                    strings = Some(Pool::from_bytes(part, header)?);
                }
                0x200 => {
                    ensure!(header >= 284, "Invalid resource package header");
                    let package = part.u32(8)?;
                    ensure!(package <= 255, "Invalid resource package ID");
                    let offset_id = if header >= 288 { part.u32(284)? } else { 0 };
                    let type_names = pool_at(part, part.u32(268)? as usize)?;
                    let keys = pool_at(part, part.u32(276)? as usize)?;
                    let mut position = header;
                    while position < part.len() {
                        let (body, kind, header) = chunk(part, position)?;
                        if kind == 0x201 {
                            ensure!(header >= 24, "Invalid resource type header");
                            let type_id = body.byte(8)?;
                            let id = u32::from(type_id)
                                .checked_add(offset_id)
                                .filter(|id| *id > 0 && *id <= 255)
                                .ok_or_else(|| anyhow!("Invalid resource type ID"))?;
                            ensure!(type_id > 0, "Invalid resource type name index");
                            let flags = body.byte(9)?;
                            ensure!(
                                flags & !3 == 0 && flags & 3 != 3,
                                "Invalid resource offset flags"
                            );
                            let count = body.u32(12)? as usize;
                            let start = body.u32(16)? as usize;
                            let width = if flags & 2 != 0 { 2 } else { 4 };
                            ensure!(
                                start >= header
                                    && start <= body.len()
                                    && count <= 65536
                                    && count <= (start - header) / width,
                                "Invalid resource entry table"
                            );
                            if flags & 1 != 0 {
                                for index in 1..count {
                                    ensure!(
                                        body.u16(header + (index - 1) * 4)?
                                            < body.u16(header + index * 4)?,
                                        "Unordered sparse resource IDs"
                                    );
                                }
                            }
                            type_count += 1;
                            ensure!(
                                type_count <= 65536,
                                "Resource configuration budget exceeded"
                            );
                            types
                                .entry(((package as u16) << 8) | id as u16)
                                .or_default()
                                .push(Type {
                                    bytes: body,
                                    header,
                                    count,
                                    start,
                                    flags,
                                    score: score(body, header)?,
                                    type_name: type_names,
                                    keys,
                                    type_id,
                                });
                        }
                        position += body.len();
                    }
                    packages += 1;
                }
                _ => {}
            }
            offset += part.len();
        }
        ensure!(packages == expected, "Resource package count mismatch");
        for configurations in types.values_mut() {
            configurations.sort_by_key(|table| std::cmp::Reverse(table.score));
        }
        Ok(Self {
            strings: strings.ok_or_else(|| anyhow!("Missing resource value pool"))?,
            types,
            resolved: RefCell::new(HashMap::new()),
        })
    }
    fn value(&self, id: u32) -> Result<Option<Value<'_, 'a>>> {
        let Some(types) = self.types.get(&((id >> 16) as u16)) else {
            return Ok(None);
        };
        for table in types {
            if let Some(value) = table.entry((id & 0xffff) as usize)? {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }
    pub(super) fn name(&self, id: u32) -> Result<Option<String>> {
        self.value(id)?
            .map(|value| value.table.name(value.key))
            .transpose()
            .map(Option::flatten)
    }
    pub(super) fn resolve(&self, id: u32) -> Result<Option<String>> {
        if let Some(value) = self.resolved.borrow().get(&id) {
            return Ok(value.clone());
        }
        let mut current = id;
        let mut seen = HashSet::new();
        let value = loop {
            ensure!(
                seen.len() < 128 && seen.insert(current),
                "Cyclic or excessive resource references"
            );
            if let Some(style) = framework::style(current) {
                break Some(format!("@android:style/{style}"));
            }
            let Some(value) = self.value(current)? else {
                break if current == id {
                    None
                } else {
                    Some(format!("@0x{current:08x}"))
                };
            };
            if value.complex {
                break value.table.name(value.key)?;
            }
            if matches!(value.kind, 1 | 7) {
                current = value.data;
                continue;
            }
            break Some(xml::typed(&self.strings, value.kind, value.data)?);
        };
        self.resolved.borrow_mut().insert(id, value.clone());
        Ok(value)
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/services/packages/resources.rs"]
pub(super) mod tests;
