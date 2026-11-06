//! Tables as delimited text: CSV ([RFC 4180](https://www.rfc-editor.org/rfc/rfc4180)) and its
//! tab-separated variant.
//!
//! Every parser in the family records a merged cell once, on the cell that owns it: the
//! positions it covers — the tail of a horizontal span, and the columns a vertical span occupies
//! in the rows below — have no cell of their own. Laying such a table on its grid, and quoting
//! the fields so a spreadsheet or a CSV reader gets the values back, are the same work whatever
//! the document format was, so it is done here once.
//!
//! The layout: a merged cell's text sits at its top-left position and the positions it covers
//! are empty, so every record has the same number of fields and no value is counted twice (the
//! shape pandas reads as one value and blanks). Rows are written as given — a table's header
//! rows first, as they are.
//!
//! `std`-only, like the rest of the crate root.

/// One cell of a table row as the family's models record it: its text, and how many rows and
/// columns it owns. A span of 0 is read as 1.
///
/// The text is the cell's plain text; a cell of several paragraphs joins them with `\n`, which
/// a field keeps (quoted) rather than flattening.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell<S> {
    /// The cell's plain text.
    pub text: S,
    /// Rows the cell owns, its own included.
    pub row_span: u32,
    /// Columns the cell owns, its own included.
    pub col_span: u32,
}

impl<S> Cell<S> {
    /// A cell owning `row_span` × `col_span` grid positions.
    pub fn new(text: S, row_span: u32, col_span: u32) -> Self {
        Self {
            text,
            row_span,
            col_span,
        }
    }

    /// A cell owning only its own position.
    #[cfg(test)]
    pub fn single(text: S) -> Self {
        Self::new(text, 1, 1)
    }
}

/// The table as CSV: one record per row, fields separated by `,`, records ended by CRLF
/// (RFC 4180 §2). See [`to_delimited`].
///
/// ```
/// use crate::csv::{to_csv, Cell};
///
/// let rows = [
///     vec![Cell::single("Item"), Cell::single("Note")],
///     vec![Cell::single("Bolt, M6"), Cell::single("He said \"no\"")],
/// ];
/// assert_eq!(to_csv(rows), "Item,Note\r\n\"Bolt, M6\",\"He said \"\"no\"\"\"\r\n");
/// ```
pub fn to_csv<R, C, S>(rows: R) -> String
where
    R: IntoIterator<Item = C>,
    C: IntoIterator<Item = Cell<S>>,
    S: AsRef<str>,
{
    to_delimited(rows, ',')
}

/// The table as delimited text with `delimiter` between fields (`'\t'` for TSV), laid out on
/// its grid as the [module](self) describes.
///
/// A field holding the delimiter, a double quote or a line break is enclosed in double quotes,
/// with each quote inside doubled; every other field is written as it is.
///
/// ```
/// use crate::csv::{to_delimited, Cell};
///
/// // ┌────────┬───────────┐
/// // │ Region │   Sales   │
/// // │        ├─────┬─────┤
/// // │        │2024 │2025 │
/// // ├────────┼─────┼─────┤
/// // │ North  │ 10  │ 12  │
/// // └────────┴─────┴─────┘
/// let rows = [
///     vec![Cell::new("Region", 2, 1), Cell::new("Sales", 1, 2)],
///     vec![Cell::single("2024"), Cell::single("2025")],
///     vec![Cell::single("North"), Cell::single("10"), Cell::single("12")],
/// ];
/// assert_eq!(
///     to_delimited(rows, '\t'),
///     "Region\tSales\t\r\n\t2024\t2025\r\nNorth\t10\t12\r\n"
/// );
/// ```
pub fn to_delimited<R, C, S>(rows: R, delimiter: char) -> String
where
    R: IntoIterator<Item = C>,
    C: IntoIterator<Item = Cell<S>>,
    S: AsRef<str>,
{
    let mut out = String::new();
    for record in grid(rows) {
        for (i, field) in record.iter().enumerate() {
            if i > 0 {
                out.push(delimiter);
            }
            push_field(&mut out, field, delimiter);
        }
        out.push_str("\r\n");
    }
    out
}

fn push_field(out: &mut String, field: &str, delimiter: char) {
    let quote = field.contains(delimiter)
        || field.contains('"')
        || field.contains('\n')
        || field.contains('\r');
    if quote {
        out.push('"');
        out.push_str(&field.replace('"', "\"\""));
        out.push('"');
    } else {
        out.push_str(field);
    }
}

/// The rows laid out on their grid ([`crate::grid::place`]): each merged cell's text at its
/// top-left position, the positions it covers empty, every record as wide as the table.
fn grid<R, C, S>(rows: R) -> Vec<Vec<String>>
where
    R: IntoIterator<Item = C>,
    C: IntoIterator<Item = Cell<S>>,
    S: AsRef<str>,
{
    let rows: Vec<Vec<Cell<S>>> = rows.into_iter().map(|r| r.into_iter().collect()).collect();
    let placement = crate::grid::place(rows.iter().map(|row| {
        row.iter()
            .map(|cell| crate::grid::Span::new(cell.row_span, cell.col_span))
    }));
    let mut grid = vec![vec![String::new(); placement.width]; rows.len()];
    for ((record, row), columns) in grid.iter_mut().zip(&rows).zip(&placement.columns) {
        for (cell, &col) in row.iter().zip(columns) {
            record[col] = cell.text.as_ref().to_string();
        }
    }
    grid
}

#[cfg(test)]
#[path = "../tests/unit/csv.rs"]
mod tests;
