use crate::{Cell, Document, Error, Result, Row, Section, Table, limits};

/// Parse UTF-8 CSV with an optional BOM. CSV has no magic bytes; callers must know the format.
pub fn parse_csv(bytes: &[u8]) -> Result<Document> {
    limits::check_input(bytes.len())?;
    let input =
        std::str::from_utf8(bytes).map_err(|_| Error::InvalidData("CSV must be UTF-8".into()))?;
    let mut chars = input
        .strip_prefix('\u{feff}')
        .unwrap_or(input)
        .chars()
        .peekable();
    let mut table = Table::new();
    let mut columns = 0;
    while chars.peek().is_some() {
        let mut row = Row::new();
        loop {
            columns = columns.max(row.cells.len() + 1);
            limits::check_table_grid(columns, table.rows.len() + 1)?;
            let mut value = String::new();
            if chars.next_if_eq(&'"').is_some() {
                loop {
                    match chars.next() {
                        Some('"') if chars.next_if_eq(&'"').is_some() => value.push('"'),
                        Some('"') => break,
                        Some(c) => value.push(c),
                        None => return Err(Error::InvalidData("Unclosed CSV quote".into())),
                    }
                }
            } else {
                while let Some(c) = chars.next_if(|c| !matches!(c, ',' | '\r' | '\n')) {
                    value.push(c);
                }
            }
            row.add_cell(if value.is_empty() {
                Cell::new()
            } else {
                Cell::with_text(value)
            });
            match chars.next() {
                Some(',') => continue,
                Some('\r') => {
                    chars.next_if_eq(&'\n');
                }
                Some('\n') | None => {}
                Some(_) => {
                    return Err(Error::InvalidData("Unexpected text after CSV quote".into()));
                }
            }
            break;
        }
        table.add_row(row);
    }
    let mut document = Document::new();
    if !table.is_empty() {
        let mut section = Section::new(0);
        section.add_table(table);
        document.add_section(section);
    }
    Ok(document)
}
