use super::*;

thread_local! {
    static PEEKS: std::cell::RefCell<Option<Vec<usize>>> = const { std::cell::RefCell::new(None) };
}

pub(super) fn record_peek(index: usize) {
    PEEKS.with_borrow_mut(|counts| {
        if let Some(counts) = counts {
            counts.resize(counts.len().max(index + 1), 0);
            counts[index] += 1;
        }
    });
}

#[test]
fn number_terminators_are_not_read_twice_by_the_number_parser() {
    let input = b"[1,22,333]";
    PEEKS.set(Some(Vec::new()));
    let value: crate::serde_json::Value = crate::serde_json::from_slice(input).unwrap();
    let counts = PEEKS.take().unwrap();
    assert_eq!(value, crate::serde_json::json!([1, 22, 333]));
    // One peek ends the number and one starts the next sequence item.
    assert_eq!(counts[2], 2);
    assert_eq!(counts[5], 2);
}
