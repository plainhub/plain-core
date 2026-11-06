//! Document properties of the 97-2003 binary formats: the `\u{5}SummaryInformation` property
//! set ([MS-OLEPS]) every one of them carries, read into the same [`Metadata`] the Office Open
//! XML readers fill from `docProps/core.xml` and `app.xml`.

use std::io::{Read, Seek};

use crate::model::Metadata;

const STREAM: &str = "/\u{5}SummaryInformation";

const VT_I2: u16 = 0x0002;
const VT_I4: u16 = 0x0003;
const VT_LPSTR: u16 = 0x001E;
const VT_LPWSTR: u16 = 0x001F;
const VT_FILETIME: u16 = 0x0040;

const PID_CODEPAGE: u32 = 1;
const PID_TITLE: u32 = 2;
const PID_SUBJECT: u32 = 3;
const PID_AUTHOR: u32 = 4;
const PID_KEYWORDS: u32 = 5;
const PID_COMMENTS: u32 = 6;
const PID_LAST_AUTHOR: u32 = 8;
const PID_CREATED: u32 = 12;
const PID_SAVED: u32 = 13;
const PID_PAGE_COUNT: u32 = 14;
const PID_WORD_COUNT: u32 = 15;
const PID_APP_NAME: u32 = 18;

/// Read a container's summary properties. Absent or unreadable properties leave the metadata
/// empty: they describe the document, they are never a reason not to read it.
pub(crate) fn read<F: Read + Seek>(container: &mut cfb::CompoundFile<F>) -> Metadata {
    let mut data = Vec::new();
    let read = container
        .open_stream(STREAM)
        .and_then(|mut stream| stream.read_to_end(&mut data));
    match read {
        Ok(_) => parse(&data),
        Err(_) => Metadata::default(),
    }
}

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    d.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    d.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// One property's value.
enum Value {
    Int(i64),
    Text(String),
    Time(String),
}

fn parse(data: &[u8]) -> Metadata {
    let mut meta = Metadata::default();
    // PropertySetStream: byte order, version, system id, CLSID, set count, then the first
    // set's FMTID and offset — SummaryInformation is the first (and only) set.
    let Some(section) = u32_at(data, 44).map(|o| o as usize) else {
        return meta;
    };
    let Some(count) = u32_at(data, section + 4) else {
        return meta;
    };
    let entries: Vec<(u32, usize)> = (0..count.min(1024) as usize)
        .filter_map(|i| {
            let at = section + 8 + i * 8;
            Some((u32_at(data, at)?, section + u32_at(data, at + 4)? as usize))
        })
        .collect();

    // Strings are in the set's code page, which is itself a property.
    let codepage = entries
        .iter()
        .find(|(id, _)| *id == PID_CODEPAGE)
        .and_then(|&(_, at)| u16_at(data, at + 4))
        .unwrap_or(1252);

    for &(id, at) in &entries {
        let Some(value) = value_at(data, at, codepage) else {
            continue;
        };
        match (id, value) {
            (PID_TITLE, Value::Text(t)) => meta.title = Some(t),
            (PID_SUBJECT, Value::Text(t)) => meta.subject = Some(t),
            (PID_AUTHOR, Value::Text(t)) => meta.author = Some(t),
            (PID_COMMENTS, Value::Text(t)) => meta.description = Some(t),
            (PID_LAST_AUTHOR, Value::Text(t)) => meta.last_modified_by = Some(t),
            (PID_APP_NAME, Value::Text(t)) => meta.application = Some(t),
            (PID_KEYWORDS, Value::Text(t)) => {
                meta.keywords = t
                    .split([';', ','])
                    .map(str::trim)
                    .filter(|k| !k.is_empty())
                    .map(str::to_string)
                    .collect();
            }
            (PID_CREATED, Value::Time(t)) => meta.created = Some(t),
            (PID_SAVED, Value::Time(t)) => meta.modified = Some(t),
            (PID_PAGE_COUNT, Value::Int(n)) if n > 0 => meta.page_count = Some(n as u32),
            (PID_WORD_COUNT, Value::Int(n)) if n > 0 => meta.word_count = Some(n as u32),
            _ => {}
        }
    }
    meta
}

fn value_at(data: &[u8], at: usize, codepage: u16) -> Option<Value> {
    let kind = u16_at(data, at)?;
    let body = at + 4;
    match kind {
        VT_I2 => Some(Value::Int(u16_at(data, body)? as i16 as i64)),
        VT_I4 => Some(Value::Int(u32_at(data, body)? as i32 as i64)),
        VT_LPSTR => {
            let len = u32_at(data, body)? as usize;
            let bytes = data.get(body + 4..body + 4 + len)?;
            let text = crate::codepage::decode_strict(bytes, codepage)?;
            let text = text.trim_end_matches('\0').trim().to_string();
            (!text.is_empty()).then_some(Value::Text(text))
        }
        VT_LPWSTR => {
            let count = u32_at(data, body)? as usize;
            let units: Vec<u16> = data
                .get(body + 4..body + 4 + count * 2)?
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&c| u16::from_le_bytes(c))
                .take_while(|&u| u != 0)
                .collect();
            let text = String::from_utf16_lossy(&units).trim().to_string();
            (!text.is_empty()).then_some(Value::Text(text))
        }
        VT_FILETIME => {
            let ticks = u32_at(data, body)? as u64 | (u32_at(data, body + 4)? as u64) << 32;
            filetime(ticks).map(Value::Time)
        }
        _ => None,
    }
}

/// A FILETIME (100 ns ticks since 1601-01-01 UTC) as `YYYY-MM-DDThh:mm:ssZ`, the form the
/// .docx reader reports. Zero — "never set" — is none.
fn filetime(ticks: u64) -> Option<String> {
    const UNIX_EPOCH_SECONDS: i64 = 11_644_473_600;
    if ticks == 0 {
        return None;
    }
    let secs = (ticks / 10_000_000) as i64 - UNIX_EPOCH_SECONDS;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    ))
}

#[cfg(test)]
#[path = "../tests/unit/summary.rs"]
mod tests;
