use crate::{Error, Result};
use std::io::Read;

pub const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const MAX_PART_BYTES: u64 = 16 * 1024 * 1024;
pub(crate) const MAX_DOCUMENT_BYTES: u64 = 64 * 1024 * 1024;
pub(crate) const MAX_PARTS: usize = 4096;
pub(crate) const MAX_XML_DEPTH: usize = 128;

pub(crate) fn read_input(input: impl Read) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    input
        .take(MAX_INPUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    check_input(bytes.len())?;
    Ok(bytes)
}

pub(crate) fn check_input(size: usize) -> Result<()> {
    if size > MAX_INPUT_BYTES {
        return Err(Error::InvalidData(
            "Document exceeds the 32 MiB input limit".into(),
        ));
    }
    Ok(())
}

pub(crate) fn check_xml(xml: &str) -> Result<()> {
    let mut reader = plain_libs::xml::Reader::from_str(xml);
    let mut depth = 0;
    while reader.buffer_position() < xml.len() as u64 {
        let event = reader.read_event()?;
        match event {
            plain_libs::xml::events::Event::Start(_) => {
                depth += 1;
                if depth > MAX_XML_DEPTH {
                    return Err(Error::InvalidData(
                        "Document XML nesting limit exceeded".into(),
                    ));
                }
            }
            plain_libs::xml::events::Event::End(_) => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| Error::InvalidData("Unexpected XML closing tag".into()))?;
            }
            plain_libs::xml::events::Event::DocType(_) => {
                return Err(Error::InvalidData(
                    "Document XML DTD is not supported".into(),
                ));
            }
            plain_libs::xml::events::Event::Eof => break,
            _ => {}
        }
    }
    if depth != 0 {
        return Err(Error::InvalidData("Unclosed document XML element".into()));
    }
    Ok(())
}

pub(crate) fn check_document(document: &crate::Document) -> Result<()> {
    for section in &document.sections {
        for block in &section.content {
            if let crate::Block::Table(table) = block {
                check_table(table, 0)?;
            }
        }
    }
    Ok(())
}

fn check_table(table: &crate::Table, depth: usize) -> Result<()> {
    if depth > MAX_XML_DEPTH {
        return Err(Error::InvalidData(
            "Document table nesting limit exceeded".into(),
        ));
    }
    let columns = table
        .rows
        .iter()
        .map(|row| {
            row.cells
                .iter()
                .map(|cell| cell.col_span.clamp(1, crate::grid::MAX_COL_SPAN) as usize)
                .sum::<usize>()
        })
        .max()
        .unwrap_or(0);
    if columns.saturating_mul(table.rows.len()) > 1_000_000 {
        return Err(Error::InvalidData(
            "Document table grid limit exceeded".into(),
        ));
    }
    if table.column_count().saturating_mul(table.rows.len()) > 1_000_000 {
        return Err(Error::InvalidData(
            "Document merged table grid limit exceeded".into(),
        ));
    }
    for cell in table.rows.iter().flat_map(|row| &row.cells) {
        for nested in &cell.nested_tables {
            check_table(nested, depth + 1)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/limits.rs"]
mod tests;
