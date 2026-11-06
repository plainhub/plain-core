use super::*;

fn csv(rows: &[&[Cell<&str>]]) -> String {
    to_csv(rows.iter().map(|row| row.iter().copied()))
}

#[test]
fn plain_fields_are_written_as_they_are() {
    assert_eq!(
        csv(&[&[Cell::single("a"), Cell::single("b c")]]),
        "a,b c\r\n"
    );
}

#[test]
fn a_field_with_a_delimiter_quote_or_line_break_is_quoted() {
    assert_eq!(
        csv(&[&[
            Cell::single("1,5"),
            Cell::single("say \"hi\""),
            Cell::single("two\nlines"),
            Cell::single("cr\rhere"),
        ]]),
        "\"1,5\",\"say \"\"hi\"\"\",\"two\nlines\",\"cr\rhere\"\r\n"
    );
}

#[test]
fn the_delimiter_decides_what_needs_quoting() {
    let row = [Cell::single("a,b"), Cell::single("c\td")];
    assert_eq!(to_delimited([row], '\t'), "a,b\t\"c\td\"\r\n");
}

#[test]
fn a_horizontal_span_leaves_the_positions_it_covers_empty() {
    assert_eq!(
        csv(&[
            &[Cell::new("wide", 1, 3)],
            &[Cell::single("a"), Cell::single("b"), Cell::single("c")],
        ]),
        "wide,,\r\na,b,c\r\n"
    );
}

#[test]
fn a_vertical_span_pushes_the_rows_below_rightward() {
    // The second and third rows have no cell for column 0: the merge owns it.
    assert_eq!(
        csv(&[
            &[Cell::new("tall", 3, 1), Cell::single("a")],
            &[Cell::single("b")],
            &[Cell::single("c")],
            &[Cell::single("d"), Cell::single("e")],
        ]),
        "tall,a\r\n,b\r\n,c\r\nd,e\r\n"
    );
}

#[test]
fn a_cell_merged_down_at_the_row_end_leaves_the_next_row_aligned() {
    assert_eq!(
        csv(&[
            &[Cell::single("a"), Cell::new("tall", 2, 1)],
            &[Cell::single("b")],
            &[Cell::single("c"), Cell::single("d")],
        ]),
        "a,tall\r\nb,\r\nc,d\r\n"
    );
}

#[test]
fn a_block_merge_covers_rows_and_columns() {
    assert_eq!(
        csv(&[
            &[Cell::new("block", 2, 2), Cell::single("x")],
            &[Cell::single("y")],
            &[Cell::single("p"), Cell::single("q"), Cell::single("r")],
        ]),
        "block,,x\r\n,,y\r\np,q,r\r\n"
    );
}

#[test]
fn short_rows_are_padded_to_the_widest() {
    assert_eq!(
        csv(&[
            &[Cell::single("a")],
            &[Cell::single("b"), Cell::single("c")]
        ]),
        "a,\r\nb,c\r\n"
    );
}

#[test]
fn a_zero_span_is_read_as_one() {
    assert_eq!(
        csv(&[
            &[Cell::new("z", 0, 0), Cell::single("a")],
            &[Cell::single("b"), Cell::single("c")]
        ]),
        "z,a\r\nb,c\r\n"
    );
}

#[test]
fn an_empty_table_is_empty_text() {
    assert_eq!(csv(&[]), "");
}

#[test]
fn owned_text_works_as_well_as_borrowed() {
    let rows = vec![vec![Cell::single(String::from("own"))]];
    assert_eq!(to_csv(rows), "own\r\n");
}
