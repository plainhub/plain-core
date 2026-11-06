//! Laying a table's cells out on a flat column grid.
//!
//! The document model records a merge as a span on the cell that owns it, which is how
//! the source formats describe it. Markdown and the plain-text table have no equivalent:
//! every row there is a flat sequence of columns. So a merged cell has to be placed at
//! the column it starts in and the columns it covers filled with empty ones.
//!
//! Without that step a row holding four merged group labels is four columns wide instead
//! of seventeen, and whatever the renderer does to reconcile that — pad one end or the
//! other — moves every value in the row out from under its heading.

use crate::model::{Cell, Table};

/// Place a table's cells on a grid: one `Vec` per row, one entry per column.
///
/// `None` is a column covered by a merge — the tail of a horizontal span, or a column a
/// vertical span occupies from a row above — and renders as an empty cell. Every row
/// comes back the same width, so callers do not pad.
///
/// Cells are placed at [`Table::cell_columns`], which is also where the grid's width
/// comes from: a vertical span pushes later cells rightward, so a row can need more
/// columns than its own spans sum to.
pub(super) fn lay_out(table: &Table) -> Vec<Vec<Option<&Cell>>> {
    let width = table.column_count();
    table
        .cell_columns()
        .into_iter()
        .zip(&table.rows)
        .map(|(cols, row)| {
            let mut slots: Vec<Option<&Cell>> = vec![None; width];
            for (col, cell) in cols.into_iter().zip(&row.cells) {
                slots[col] = Some(cell);
            }
            slots
        })
        .collect()
}

#[cfg(test)]
#[path = "../../tests/unit/render/grid.rs"]
mod tests;
