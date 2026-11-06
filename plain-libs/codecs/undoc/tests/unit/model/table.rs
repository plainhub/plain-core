use super::*;

#[test]
fn test_cell_creation() {
    let cell = Cell::with_text("Hello");
    assert_eq!(cell.plain_text(), "Hello");
    assert!(!cell.is_empty());
    assert!(!cell.has_spans());
}

#[test]
fn test_cell_spans() {
    let mut cell = Cell::with_text("Merged");
    cell.col_span = 2;
    cell.row_span = 3;
    assert!(cell.has_col_span());
    assert!(cell.has_row_span());
    assert!(cell.has_spans());
}

fn spanned(text: &str, col_span: u32, row_span: u32) -> Cell {
    Cell {
        col_span,
        row_span,
        ..Cell::with_text(text)
    }
}

#[test]
fn test_cell_columns_follow_spans() {
    // | A (2 rows) | B (2 cols)  |
    // |            | c  | d      |
    // | e          | f  | g      |
    let mut table = Table::new();
    table.add_row(Row {
        cells: vec![spanned("A", 1, 2), spanned("B", 2, 1)],
        ..Row::new()
    });
    table.add_row(Row {
        cells: vec![Cell::with_text("c"), Cell::with_text("d")],
        ..Row::new()
    });
    table.add_row(Row {
        cells: vec![
            Cell::with_text("e"),
            Cell::with_text("f"),
            Cell::with_text("g"),
        ],
        ..Row::new()
    });
    assert_eq!(
        table.cell_columns(),
        vec![vec![0, 1], vec![1, 2], vec![0, 1, 2]]
    );
    assert_eq!(table.column_count(), 3);
}

#[test]
fn test_a_declared_span_past_the_bound_does_not_size_the_table() {
    // A `w:gridSpan` is a decimal attribute; one cell declaring billions of columns must
    // not make every laid-out row that wide (`crate::grid::MAX_COL_SPAN`).
    let mut table = Table::new();
    table.add_row(Row {
        cells: vec![Cell {
            col_span: u32::MAX,
            ..Cell::with_text("wide")
        }],
        ..Row::new()
    });
    assert_eq!(table.column_count(), crate::grid::MAX_COL_SPAN as usize);
    assert!(table.to_csv().len() < 2_000);
}

#[test]
fn test_column_count_includes_columns_pushed_right_by_a_vertical_span() {
    // Row 2 sums to two columns but starts in column 1.
    let mut table = Table::new();
    table.add_row(Row {
        cells: vec![spanned("A", 1, 2), Cell::with_text("b")],
        ..Row::new()
    });
    table.add_row(Row {
        cells: vec![Cell::with_text("c"), Cell::with_text("d")],
        ..Row::new()
    });
    assert_eq!(table.column_count(), 3);
}

#[test]
fn test_row_creation() {
    let mut row = Row::new();
    row.add_cell(Cell::with_text("A"));
    row.add_cell(Cell::with_text("B"));
    assert_eq!(row.len(), 2);
    assert_eq!(row.effective_columns(), 2);
}

#[test]
fn test_row_with_spans() {
    let mut row = Row::new();
    let mut cell = Cell::with_text("Merged");
    cell.col_span = 2;
    row.add_cell(cell);
    row.add_cell(Cell::with_text("Single"));
    assert_eq!(row.len(), 2);
    assert_eq!(row.effective_columns(), 3);
}

#[test]
fn test_table_creation() {
    let mut table = Table::new();
    let mut header = Row::new();
    header.add_cell(Cell::header("Name"));
    header.add_cell(Cell::header("Value"));
    header.is_header = true;
    table.add_row(header);

    let mut row = Row::new();
    row.add_cell(Cell::with_text("foo"));
    row.add_cell(Cell::with_text("bar"));
    table.add_row(row);

    assert_eq!(table.row_count(), 2);
    assert_eq!(table.column_count(), 2);
    assert_eq!(table.header_rows().len(), 1);
    assert_eq!(table.data_rows().len(), 1);
}

#[test]
fn test_table_has_merged_cells() {
    let mut table = Table::new();
    let mut row = Row::new();
    row.add_cell(Cell::with_text("Normal"));
    table.add_row(row);
    assert!(!table.has_merged_cells());

    let mut row2 = Row::new();
    let mut merged = Cell::with_text("Merged");
    merged.col_span = 2;
    row2.add_cell(merged);
    table.add_row(row2);
    assert!(table.has_merged_cells());
}

#[test]
fn test_table_plain_text() {
    let mut table = Table::new();
    let mut row = Row::new();
    row.add_cell(Cell::with_text("A1"));
    row.add_cell(Cell::with_text("B1"));
    table.add_row(row);

    let text = table.plain_text();
    assert!(text.contains("A1"));
    assert!(text.contains("B1"));
}

#[test]
fn test_row_is_empty_with_content_empty_cells() {
    let mut row = Row::new();
    row.add_cell(Cell::new()); // empty cell, no text
    row.add_cell(Cell::new());
    // Row has cells but all are content-empty — should be considered empty
    assert!(
        row.is_empty(),
        "row with only empty cells should be is_empty()"
    );
}

#[test]
fn test_row_is_empty_false_when_cell_has_content() {
    let mut row = Row::new();
    row.add_cell(Cell::with_text("hello"));
    assert!(!row.is_empty());
}

#[test]
fn test_row_is_empty_true_with_no_cells() {
    let row = Row::new();
    assert!(row.is_empty());
}

fn row(cells: Vec<Cell>) -> Row {
    Row {
        cells,
        ..Row::default()
    }
}

fn spanning(text: &str, row_span: u32, col_span: u32) -> Cell {
    Cell {
        row_span,
        col_span,
        ..Cell::with_text(text)
    }
}

#[test]
fn to_csv_lays_merged_cells_on_their_grid() {
    // A vertical merge owns column 0 in the row below, which has no cell for it.
    let mut table = Table::new();
    table.add_row(row(vec![spanning("Region", 2, 1), spanning("Sales", 1, 2)]));
    table.add_row(row(vec![Cell::with_text("2024"), Cell::with_text("2025")]));
    table.add_row(row(vec![
        Cell::with_text("North"),
        Cell::with_text("10"),
        Cell::with_text("12"),
    ]));
    assert_eq!(
        table.to_csv(),
        "Region,Sales,\r\n,2024,2025\r\nNorth,10,12\r\n"
    );
}

#[test]
fn to_csv_keeps_a_cells_paragraphs_and_leaves_nested_tables_out() {
    let mut cell = Cell::with_text("first");
    cell.content.push(Paragraph::with_text("second"));
    let mut inner = Table::new();
    inner.add_row(row(vec![Cell::with_text("inner")]));
    cell.nested_tables.push(inner);
    let mut table = Table::new();
    table.add_row(row(vec![cell, Cell::with_text("x\ty")]));
    assert_eq!(table.to_csv(), "\"first\nsecond\",x\ty\r\n");
    assert_eq!(table.to_delimited('\t'), "\"first\nsecond\"\t\"x\ty\"\r\n");
}
