use plain_libs::anyhow::{Context, Result, bail, ensure};
use std::io::{ErrorKind, Read};
use std::path::Path;

#[derive(Default)]
pub(super) struct Preferences {
    pub client_id: Option<String>,
    pub signature_key_pair: Option<String>,
    pub scan_history: Option<String>,
    pub pomodoro_settings: Option<String>,
    pub favorite_folders: Option<String>,
    pub quick_note_draft: Option<String>,
}

pub(super) fn read(path: &Path) -> Result<Preferences> {
    read_bytes(path)?
        .as_deref()
        .map(decode)
        .transpose()
        .map(|values| values.unwrap_or_default())
        .context("Invalid legacy settings file")
}

fn read_bytes(path: &Path) -> Result<Option<Vec<u8>>> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    const MAX_BYTES: usize = 64 * 1024 * 1024;
    let mut bytes = Vec::new();
    file.take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= MAX_BYTES,
        "Legacy settings file is too large"
    );
    Ok(Some(bytes))
}

pub(super) fn clear(path: &Path) -> Result<()> {
    use std::io::Write;
    let Some(bytes) = read_bytes(path)? else {
        return Ok(());
    };
    let mut remaining = bytes.as_slice();
    let mut retained = Vec::new();
    while !remaining.is_empty() {
        let start = remaining;
        let (number, value) = field(&mut remaining)?;
        let imported = if let (1, Some(entry)) = (number, value) {
            matches!(
                map_entry(entry)?.0,
                Some(
                    "client_id"
                        | "signature_key_pair"
                        | "scan_history"
                        | "pomodoro_settings"
                        | "favorite_folders"
                        | "quick_note_draft"
                )
            )
        } else {
            false
        };
        if !imported {
            retained.extend_from_slice(&start[..start.len() - remaining.len()]);
        }
    }
    if retained.len() != bytes.len() {
        let mut output = plain_libs::temporary::NamedTempFile::new_in(
            path.parent().context("Missing legacy settings directory")?,
        )?;
        output.write_all(&retained)?;
        output.as_file().sync_all()?;
        output.persist(path)?;
    }
    Ok(())
}

fn decode(bytes: &[u8]) -> Result<Preferences> {
    let mut decoded = Preferences::default();
    length_delimited_fields(bytes, |number, entry| {
        if number != 1 {
            return Ok(());
        }
        let (name, value) = map_entry(entry)?;
        let slot = match name {
            Some("client_id") => &mut decoded.client_id,
            Some("signature_key_pair") => &mut decoded.signature_key_pair,
            Some("scan_history") => &mut decoded.scan_history,
            Some("pomodoro_settings") => &mut decoded.pomodoro_settings,
            Some("favorite_folders") => &mut decoded.favorite_folders,
            Some("quick_note_draft") => &mut decoded.quick_note_draft,
            _ => return Ok(()),
        };
        let mut string = None;
        length_delimited_fields(value.unwrap_or_default(), |number, bytes| {
            if number == 5 {
                string = Some(std::str::from_utf8(bytes)?.to_owned());
            }
            Ok(())
        })?;
        let string = string.context("Legacy string preference has an invalid type")?;
        *slot = (!string.is_empty()).then_some(string);
        Ok(())
    })?;
    Ok(decoded)
}

fn map_entry(entry: &[u8]) -> Result<(Option<&str>, Option<&[u8]>)> {
    let (mut name, mut value) = (None, None);
    length_delimited_fields(entry, |number, bytes| {
        match number {
            1 => name = Some(std::str::from_utf8(bytes)?),
            2 => value = Some(bytes),
            _ => {}
        }
        Ok(())
    })?;
    Ok((name, value))
}

fn length_delimited_fields<'a>(
    mut bytes: &'a [u8],
    mut consume: impl FnMut(u64, &'a [u8]) -> Result<()>,
) -> Result<()> {
    while !bytes.is_empty() {
        let (number, value) = field(&mut bytes)?;
        if let Some(value) = value {
            consume(number, value)?;
        }
    }
    Ok(())
}

fn field<'a>(bytes: &mut &'a [u8]) -> Result<(u64, Option<&'a [u8]>)> {
    let tag = varint(bytes)?;
    ensure!(
        tag >> 3 != 0 && tag <= u32::MAX as u64,
        "Invalid settings field tag"
    );
    match tag & 7 {
        0 => {
            varint(bytes)?;
        }
        1 => {
            take(bytes, 8)?;
        }
        2 => {
            let len = usize::try_from(varint(bytes)?)?;
            return Ok((tag >> 3, Some(take(bytes, len)?)));
        }
        5 => {
            take(bytes, 4)?;
        }
        _ => bail!("Invalid settings field encoding"),
    }
    Ok((tag >> 3, None))
}

fn take<'a>(bytes: &mut &'a [u8], len: usize) -> Result<&'a [u8]> {
    let (field, rest) = bytes
        .split_at_checked(len)
        .context("Truncated settings field")?;
    *bytes = rest;
    Ok(field)
}

fn varint(bytes: &mut &[u8]) -> Result<u64> {
    let mut value = 0;
    for shift in (0..64).step_by(7) {
        let byte = take(bytes, 1)?[0];
        ensure!(shift != 63 || byte <= 1, "Invalid settings integer");
        value |= u64::from(byte & 127) << shift;
        if byte & 128 == 0 {
            return Ok(value);
        }
    }
    bail!("Invalid settings integer")
}
