//! Table model structures.

use super::Paragraph;
#[cfg(test)]
use crate::csv;
use serde::{Deserialize, Serialize};

/// Horizontal alignment for table cells.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CellAlignment {
    #[default]
    Left,
    Center,
    Right,
}

/// Vertical alignment for table cells.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VerticalAlignment {
    #[default]
    Top,
    Middle,
    Bottom,
}

/// A cell in a table.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Cell {
    /// Cell content (paragraphs)
    #[serde(default)]
    pub content: Vec<Paragraph>,

    /// Nested tables within this cell
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nested_tables: Vec<Table>,

    /// Horizontal span (colspan)
    #[serde(default = "default_span", skip_serializing_if = "is_default_span")]
    pub col_span: u32,

    /// Vertical span (rowspan)
    #[serde(default = "default_span", skip_serializing_if = "is_default_span")]
    pub row_span: u32,

    /// Horizontal alignment
    #[serde(default, skip_serializing_if = "is_default_cell_alignment")]
    pub alignment: CellAlignment,

    /// Vertical alignment
    #[serde(default, skip_serializing_if = "is_default_vertical_alignment")]
    pub vertical_alignment: VerticalAlignment,

    /// Whether this is a header cell
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_header: bool,

    /// Background color (hex)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,

    /// Resource id of the picture that fills the cell. A fill is not content: the cell's
    /// text stays its text and the image is only referenced from here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background_image: Option<String>,
}

fn default_span() -> u32 {
    1
}

fn is_default_span(n: &u32) -> bool {
    *n == 1
}

fn is_default_cell_alignment(a: &CellAlignment) -> bool {
    *a == CellAlignment::Left
}

fn is_default_vertical_alignment(a: &VerticalAlignment) -> bool {
    *a == VerticalAlignment::Top
}

impl Cell {
    /// Create a new empty cell.
    pub fn new() -> Self {
        Self {
            col_span: 1,
            row_span: 1,
            ..Default::default()
        }
    }

    /// Create a cell with text content.
    pub fn with_text(text: impl Into<String>) -> Self {
        Self {
            content: vec![Paragraph::with_text(text)],
            col_span: 1,
            row_span: 1,
            ..Default::default()
        }
    }

    /// Create a header cell with text.
    pub fn header(text: impl Into<String>) -> Self {
        Self {
            content: vec![Paragraph::with_text(text)],
            col_span: 1,
            row_span: 1,
            is_header: true,
            ..Default::default()
        }
    }

    /// Get the plain text content.
    pub fn plain_text(&self) -> String {
        self.content
            .iter()
            .map(|p| p.plain_text())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Check if this cell is empty.
    pub fn is_empty(&self) -> bool {
        self.content.is_empty() || self.content.iter().all(|p| p.is_empty())
    }

    /// Check if this cell spans multiple columns.
    pub fn has_col_span(&self) -> bool {
        self.col_span > 1
    }

    /// Check if this cell spans multiple rows.
    pub fn has_row_span(&self) -> bool {
        self.row_span > 1
    }

    /// Check if this cell has any spans.
    pub fn has_spans(&self) -> bool {
        self.col_span > 1 || self.row_span > 1
    }
}

/// A row in a table.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Row {
    /// Cells in this row
    #[serde(default)]
    pub cells: Vec<Cell>,

    /// Whether this is a header row
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_header: bool,

    /// Row height in twips (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

impl Row {
    /// Create a new empty row.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a header row.
    pub fn header(cells: Vec<Cell>) -> Self {
        Self {
            cells,
            is_header: true,
            height: None,
        }
    }

    /// Add a cell to this row.
    pub fn add_cell(&mut self, cell: Cell) {
        self.cells.push(cell);
    }

    /// Get the number of cells.
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Check if the row is empty — true when there are no cells or all cells are content-empty.
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty() || self.cells.iter().all(|c| c.is_empty())
    }

    /// Get the effective column count (accounting for spans).
    pub fn effective_columns(&self) -> usize {
        self.cells.iter().map(|c| c.col_span as usize).sum()
    }
}

/// A table.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Table {
    /// Rows in this table
    #[serde(default)]
    pub rows: Vec<Row>,

    /// Column widths in twips (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_widths: Option<Vec<u32>>,

    /// Table caption
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,

    /// Table style ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style_id: Option<String>,
}

impl Table {
    /// Create a new empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a row to this table.
    pub fn add_row(&mut self, row: Row) {
        self.rows.push(row);
    }

    /// Get the number of rows.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Get the number of grid columns the table occupies: the furthest any cell reaches
    /// ([`crate::grid::place`]) — not the sum of a row's spans, since a vertical span
    /// from above pushes a row's cells rightward.
    pub fn column_count(&self) -> usize {
        self.placement().width
    }

    /// The grid column each cell starts in, one `Vec` per row, parallel to `row.cells`.
    ///
    /// A merge is recorded once, on the cell that owns it: the positions it covers —
    /// the tail of a horizontal span, and the columns a vertical span occupies in the
    /// rows below — have no `Cell` of their own. So a cell's index in its row is not its
    /// column once a vertical span from above sits to its left; this walks the spans to
    /// recover it ([`crate::grid::place`]). Every parser builds tables to that
    /// contract, and every renderer that needs a flat grid places cells with this.
    pub fn cell_columns(&self) -> Vec<Vec<usize>> {
        self.placement().columns
    }

    fn placement(&self) -> crate::grid::Placement {
        crate::grid::place(self.rows.iter().map(|row| {
            row.cells
                .iter()
                .map(|cell| crate::grid::Span::new(cell.row_span, cell.col_span))
        }))
    }

    /// Check if the table is empty.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Check if the table has merged cells.
    pub fn has_merged_cells(&self) -> bool {
        self.rows
            .iter()
            .any(|r| r.cells.iter().any(|c| c.has_spans()))
    }

    /// Get the header rows.
    pub fn header_rows(&self) -> Vec<&Row> {
        self.rows.iter().filter(|r| r.is_header).collect()
    }

    /// Get the data rows (non-header).
    pub fn data_rows(&self) -> Vec<&Row> {
        self.rows.iter().filter(|r| !r.is_header).collect()
    }

    /// Get plain text representation.
    pub fn plain_text(&self) -> String {
        let mut text = String::new();
        for row in &self.rows {
            let cells: Vec<String> = row.cells.iter().map(|c| c.plain_text()).collect();
            text.push_str(&cells.join("\t"));
            text.push('\n');
        }
        text
    }

    /// The table as CSV ([RFC 4180](https://www.rfc-editor.org/rfc/rfc4180)): one record per
    /// row, in order, records ended by CRLF.
    ///
    /// A merged cell's text is in its top-left position and the positions it covers are
    /// empty, so every record has the same number of fields and a value is never counted
    /// twice. A field holding the delimiter, a quote or a line break is quoted, with quotes
    /// doubled; a cell's paragraphs are kept on their own lines inside it. A table nested in
    /// a cell is not part of the cell's text — it is a table of its own
    /// ([`Cell::nested_tables`]).
    ///
    /// ```
    /// use undoc::model::{Cell, Row, Table};
    ///
    /// let mut table = Table::new();
    /// table.add_row(Row::header(vec![Cell::header("Item"), Cell::header("Note")]));
    /// table.add_row(Row {
    ///     cells: vec![Cell::with_text("Bolt, M6"), Cell::with_text("He said \"no\"")],
    ///     ..Row::default()
    /// });
    /// assert_eq!(table.to_csv(), "Item,Note\r\n\"Bolt, M6\",\"He said \"\"no\"\"\"\r\n");
    /// ```
    #[cfg(test)]
    pub fn to_csv(&self) -> String {
        self.to_delimited(',')
    }

    /// The table as delimited text, like [`Table::to_csv`] with `delimiter` between fields
    /// (`'\t'` for TSV).
    #[cfg(test)]
    pub fn to_delimited(&self, delimiter: char) -> String {
        let rows = self.rows.iter().map(|row| {
            row.cells
                .iter()
                .map(|cell| csv::Cell::new(cell.plain_text(), cell.row_span, cell.col_span))
        });
        csv::to_delimited(rows, delimiter)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/model/table.rs"]
mod tests;
