use super::*;
use crate::model::Row;

fn row(cells: Vec<Cell>) -> Row {
    Row {
        cells,
        is_header: false,
        height: None,
    }
}

fn cell(text: &str, col_span: u32, row_span: u32) -> Cell {
    Cell {
        col_span,
        row_span,
        ..Cell::with_text(text)
    }
}

fn texts(grid: &[Vec<Option<&Cell>>]) -> Vec<Vec<String>> {
    grid.iter()
        .map(|row| {
            row.iter()
                .map(|slot| slot.map(|c| c.plain_text()).unwrap_or_default())
                .collect()
        })
        .collect()
}

#[test]
fn plain_table_is_unchanged() {
    let mut table = Table::new();
    table.add_row(row(vec![
        Cell::with_text("A"),
        Cell::with_text("B"),
        Cell::with_text("C"),
    ]));
    let grid = lay_out(&table);
    assert_eq!(texts(&grid), vec![vec!["A", "B", "C"]]);
}

#[test]
fn horizontal_span_anchors_at_its_first_column() {
    let mut table = Table::new();
    // A group label over columns 1-2, then a plain cell in column 3.
    table.add_row(row(vec![cell("Group", 2, 1), Cell::with_text("C")]));
    table.add_row(row(vec![
        Cell::with_text("a"),
        Cell::with_text("b"),
        Cell::with_text("c"),
    ]));

    let grid = texts(&lay_out(&table));
    assert_eq!(
        grid[0],
        vec!["Group", "", "C"],
        "the label must sit in column 1, not be pushed right by padding"
    );
    assert_eq!(grid[1], vec!["a", "b", "c"]);
}

#[test]
fn vertical_span_holds_its_column_in_later_rows() {
    let mut table = Table::new();
    // Column 1 spans both rows; row 2 supplies only the remaining columns.
    table.add_row(row(vec![
        cell("Side", 1, 2),
        Cell::with_text("X"),
        Cell::with_text("Y"),
    ]));
    table.add_row(row(vec![Cell::with_text("x"), Cell::with_text("y")]));

    let grid = texts(&lay_out(&table));
    assert_eq!(grid[0], vec!["Side", "X", "Y"]);
    assert_eq!(
        grid[1],
        vec!["", "x", "y"],
        "row 2's cells must start in column 2 — column 1 is still occupied"
    );
}

#[test]
fn every_row_comes_back_the_same_width() {
    let mut table = Table::new();
    table.add_row(row(vec![cell("Wide", 3, 1)]));
    table.add_row(row(vec![Cell::with_text("only one")]));

    let grid = lay_out(&table);
    assert!(grid.iter().all(|r| r.len() == 3), "got {:?}", texts(&grid));
}

#[test]
fn vertical_span_past_the_last_cell_still_widens_the_row() {
    let mut table = Table::new();
    table.add_row(row(vec![Cell::with_text("A"), cell("Tall", 1, 2)]));
    table.add_row(row(vec![Cell::with_text("b")]));

    let grid = texts(&lay_out(&table));
    assert_eq!(
        grid[1],
        vec!["b", ""],
        "the carried column belongs to row 2"
    );
}
