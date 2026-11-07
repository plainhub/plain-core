#![forbid(unsafe_code)]

pub(super) struct Record<'a> {
    pub action: u32,
    pub name: &'a [u8],
}

pub(super) fn records(mut bytes: &[u8]) -> Result<Vec<Record<'_>>, &'static str> {
    let mut records = Vec::new();
    loop {
        let header = bytes
            .get(..12)
            .ok_or("Truncated directory notification header")?;
        let next = u32::from_le_bytes(header[..4].try_into().unwrap()) as usize;
        let action = u32::from_le_bytes(header[4..8].try_into().unwrap());
        let name_len = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
        if name_len == 0 || name_len % 2 != 0 {
            return Err("Invalid directory notification filename length");
        }
        let end = 12_usize
            .checked_add(name_len)
            .ok_or("Directory notification length overflow")?;
        let name = bytes
            .get(12..end)
            .ok_or("Truncated directory notification filename")?;
        records.push(Record { action, name });
        if next == 0 {
            return Ok(records);
        }
        if next < end || next % 4 != 0 {
            return Err("Invalid directory notification offset");
        }
        bytes = bytes
            .get(next..)
            .ok_or("Directory notification offset outside buffer")?;
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/file_events/windows_records.rs"]
mod tests;
