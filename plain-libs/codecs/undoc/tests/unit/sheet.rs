use super::*;

#[test]
fn an_empty_cell_far_right_does_not_fill_the_row() {
    let mut builder = SheetTableBuilder::new(Vec::new()).unwrap();
    builder.start_row(Some(1));
    builder.place(Cell::with_text("a"), Some((0, 1))).unwrap();
    builder.place(Cell::new(), Some((16_383, 1))).unwrap();
    // Inspected before `finish`, whose trim would hide an allocation that already happened.
    assert_eq!(builder.row.as_ref().unwrap().cells.len(), 1);
    let table = builder.finish();
    assert_eq!(table.rows[0].cells.len(), 1);
}

#[test]
fn an_empty_cell_between_contents_keeps_its_column() {
    let mut builder = SheetTableBuilder::new(Vec::new()).unwrap();
    builder.start_row(Some(1));
    builder.place(Cell::with_text("a"), Some((0, 1))).unwrap();
    builder.place(Cell::new(), Some((2, 1))).unwrap();
    builder.place(Cell::with_text("e"), Some((4, 1))).unwrap();
    let table = builder.finish();
    let texts: Vec<String> = table.rows[0].cells.iter().map(|c| c.plain_text()).collect();
    assert_eq!(texts, ["a", "", "", "", "e"]);
}

#[test]
fn an_empty_merge_owner_still_takes_its_place() {
    let merge = MergeRange {
        left: 2,
        right: 3,
        top: 1,
        bottom: 1,
    };
    let mut builder = SheetTableBuilder::new(vec![merge]).unwrap();
    builder.start_row(Some(1));
    builder.place(Cell::with_text("a"), Some((0, 1))).unwrap();
    builder.place(Cell::new(), Some((2, 1))).unwrap();
    builder.place(Cell::with_text("e"), Some((4, 1))).unwrap();
    let table = builder.finish();
    let spans: Vec<u32> = table.rows[0].cells.iter().map(|c| c.col_span).collect();
    assert_eq!(spans, [1, 1, 2, 1]);
}

#[test]
fn rejects_sparse_coordinates_and_repeated_shared_text_before_expansion() {
    let mut builder = SheetTableBuilder::new(Vec::new()).unwrap();
    builder.start_row(Some(1));
    assert!(builder
        .place(Cell::with_text("text"), Some((u32::MAX, 1)))
        .is_err());
    let mut builder = SheetTableBuilder::new(Vec::new()).unwrap();
    builder.start_row(Some(1));
    builder.text_bytes = crate::limits::MAX_INPUT_BYTES;
    assert!(builder
        .place(Cell::with_text("text"), Some((0, 1)))
        .is_err());
    assert!(SheetTableBuilder::new(vec![MergeRange {
        left: 0,
        top: 1,
        right: u32::MAX,
        bottom: u32::MAX
    }])
    .is_err());
}
