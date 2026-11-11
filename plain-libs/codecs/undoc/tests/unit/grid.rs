use super::*;

#[test]
fn a_declared_span_past_the_bound_is_read_as_the_bound() {
    let placement = place(spans(&[&[(u32::MAX, u32::MAX), (1, 1)], &[(1, 1)]]));
    assert_eq!(
        placement.columns,
        vec![vec![0, MAX_COL_SPAN as usize], vec![MAX_COL_SPAN as usize]]
    );
    assert_eq!(placement.width, MAX_COL_SPAN as usize + 1);
}

fn spans(rows: &[&[(u32, u32)]]) -> Vec<Vec<Span>> {
    rows.iter()
        .map(|r| {
            r.iter()
                .map(|&(rows, cols)| Span::new(rows, cols))
                .collect()
        })
        .collect()
}

#[test]
fn plain_cells_sit_at_their_index() {
    let p = place(spans(&[&[(1, 1), (1, 1)], &[(1, 1), (1, 1)]]));
    assert_eq!(p.columns, vec![vec![0, 1], vec![0, 1]]);
    assert_eq!(p.width, 2);
}

/// A first row that merges across: its cell count is not the table's width.
#[test]
fn a_horizontal_span_in_the_first_row_widens_the_table() {
    let p = place(spans(&[&[(2, 1), (1, 2)], &[(1, 1), (1, 1)]]));
    assert_eq!(p.width, 3);
}

#[test]
fn a_vertical_span_pushes_the_rows_below_rightward() {
    let p = place(spans(&[
        &[(3, 1), (1, 1)],
        &[(1, 1)],
        &[(1, 1)],
        &[(1, 1), (1, 1)],
    ]));
    assert_eq!(p.columns, vec![vec![0, 1], vec![1], vec![1], vec![0, 1]]);
}

#[test]
fn a_cell_merged_down_at_the_row_end_is_used_up_by_the_rows_it_covers() {
    let p = place(spans(&[&[(1, 1), (2, 1)], &[(1, 1)], &[(1, 1), (1, 1)]]));
    assert_eq!(p.columns, vec![vec![0, 1], vec![0], vec![0, 1]]);
    assert_eq!(p.width, 2);
}

#[test]
fn a_block_merge_covers_rows_and_columns() {
    let p = place(spans(&[
        &[(2, 2), (1, 1)],
        &[(1, 1)],
        &[(1, 1), (1, 1), (1, 1)],
    ]));
    assert_eq!(p.columns, vec![vec![0, 2], vec![2], vec![0, 1, 2]]);
    assert_eq!(p.width, 3);
}

#[test]
fn a_zero_span_is_read_as_one() {
    let p = place(spans(&[&[(0, 0), (0, 0)]]));
    assert_eq!(p.columns, vec![vec![0, 1]]);
    assert_eq!(p.width, 2);
}

#[test]
fn an_empty_table_has_no_columns() {
    assert_eq!(place(Vec::<Vec<Span>>::new()), Placement::default());
}

#[test]
fn place_example_keeps_expected_output() {
    use crate::grid::{Span, place};

    let placement = place([
        vec![Span::new(2, 1), Span::new(1, 2)],
        vec![Span::new(1, 1), Span::new(1, 1)],
    ]);
    assert_eq!(placement.columns, vec![vec![0, 1], vec![1, 2]]);
    assert_eq!(placement.width, 3);
}
