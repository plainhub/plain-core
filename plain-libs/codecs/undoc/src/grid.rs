//! Where a table's cells sit on its grid.
//!
//! Every parser in the family records a merged cell once, on the cell that owns it: the
//! positions it covers — the tail of a horizontal span, and the columns a vertical span occupies
//! in the rows below — have no cell of their own. So a cell's index in its row is not its grid
//! column once a vertical span from a row above sits to its left, and a row's cell count is not
//! the table's width once a cell spans columns. [`place`] walks the spans to recover both — the
//! same walk whatever the document format was, done here once.
//!
//! `std`-only, like the rest of the crate root.

/// The most columns [`place`] lets one cell own: a larger span is read as this many.
///
/// A span is a number the document declares — two bytes in one format, a decimal attribute in
/// another — and the grid is as wide as the furthest any cell reaches, so one declared span
/// would otherwise size every row of the laid-out table. The value is the one HTML's table
/// processing model clamps `colspan` to (WHATWG HTML, «algorithm for processing rows»); no
/// format in the family writes a wider merge except a spreadsheet's across its whole width.
pub const MAX_COL_SPAN: u32 = 1000;

/// The most rows [`place`] lets one cell own, the value HTML clamps `rowspan` to. A row span
/// costs no allocation here — it only counts down rows that exist — but is read the same way
/// HTML reads it, so a renderer that lays rows out itself can share the bound.
pub const MAX_ROW_SPAN: u32 = 65_534;

/// How many grid rows and columns a cell owns, its own position included. A span of 0 is read
/// as 1; one past [`MAX_ROW_SPAN`] or [`MAX_COL_SPAN`] as that bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    /// Rows the cell owns.
    pub rows: u32,
    /// Columns the cell owns.
    pub cols: u32,
}

impl Span {
    /// A cell owning `rows` × `cols` grid positions.
    pub fn new(rows: u32, cols: u32) -> Self {
        Self { rows, cols }
    }
}

/// Where each cell of a table sits: see [`place`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Placement {
    /// The grid column each cell starts in, one `Vec` per row, parallel to the row's cells.
    pub columns: Vec<Vec<usize>>,
    /// How many grid columns the table occupies: the furthest any cell reaches.
    pub width: usize,
}

/// The grid column each cell starts in, and the table's width, for rows given as their cells'
/// spans in reading order.
///
/// A cell starts at the first column, from where the row has got to, that no cell merged down
/// from a row above still covers. The width is the furthest any cell reaches — which can be
/// further than a row's own spans add up to, since a vertical span from above pushes the row's
/// cells rightward.
///
pub fn place<R, C>(rows: R) -> Placement
where
    R: IntoIterator<Item = C>,
    C: IntoIterator<Item = Span>,
{
    // For each column, how many further rows a cell merged down from above still covers.
    let mut covered: Vec<usize> = Vec::new();
    let mut placement = Placement::default();
    for row in rows {
        let mut columns = Vec::new();
        let mut col = 0;
        for span in row {
            // Pass the columns a cell from above still covers; this row uses them up once.
            while covered.get(col).is_some_and(|&n| n > 0) {
                covered[col] -= 1;
                col += 1;
            }
            let (rows, cols) = (
                span.rows.clamp(1, MAX_ROW_SPAN) as usize,
                span.cols.clamp(1, MAX_COL_SPAN) as usize,
            );
            columns.push(col);
            if covered.len() < col + cols {
                covered.resize(col + cols, 0);
            }
            for c in &mut covered[col..col + cols] {
                *c = rows - 1;
            }
            col += cols;
            placement.width = placement.width.max(col);
        }
        // Columns past the row's last cell that a cell above still covers belong to this row.
        for c in covered.iter_mut().skip(col) {
            *c = c.saturating_sub(1);
        }
        placement.columns.push(columns);
    }
    placement
}

#[cfg(test)]
#[path = "../tests/unit/grid.rs"]
mod tests;
