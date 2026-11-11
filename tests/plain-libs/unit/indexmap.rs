use super::*;
#[test]
fn borrowed_keys_replacement_and_removal_preserve_order() {
    let mut map = IndexMap::new();
    for n in 0..1024 {
        map.insert(format!("key{n}"), n);
    }
    let key = "key300";
    let key_ptr = map.get_key_value(key).unwrap().0.as_ptr();
    assert_eq!(map.insert(key.to_owned(), 9999), Some(300));
    assert_eq!(map.get_key_value(key).unwrap().0.as_ptr(), key_ptr);
    assert_eq!(map.shift_remove("key301"), Some(301));
    assert_eq!(map.get_index(301).unwrap().0, "key302");
    let bytes = crate::serde_json::to_string(&map).unwrap();
    assert!(bytes.starts_with(r#"{"key0":0,"key1":1,"key2":2"#));
}

thread_local! { static SUFFIX_SHIFTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
thread_local! { static FULL_SHIFTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
pub(super) fn record_full_shift() {
    FULL_SHIFTS.set(FULL_SHIFTS.get() + 1);
}
pub(super) fn record_suffix_shift() {
    SUFFIX_SHIFTS.set(SUFFIX_SHIFTS.get() + 1);
}
#[test]
fn suffix_shift_path_preserves_dense_and_bounded_moves_and_insertion() {
    let mut map = IndexMap::new();
    let mut expected: Vec<_> = (0..256).map(|n| (n, n)).collect();
    for &(key, value) in &expected {
        map.insert(key, value);
    }
    SUFFIX_SHIFTS.set(0);
    FULL_SHIFTS.set(0);
    assert_eq!(map.shift_remove(&0), Some(0));
    expected.remove(0);
    assert_eq!(FULL_SHIFTS.get(), 1);
    map.move_index(200, 10);
    let entry = expected.remove(200);
    expected.insert(10, entry);
    map.move_index(20, 220);
    let entry = expected.remove(20);
    expected.insert(220, entry);
    let shifts_before_insert = SUFFIX_SHIFTS.get();
    map.shift_insert(1, 999, 999);
    assert!(SUFFIX_SHIFTS.get() > shifts_before_insert);
    expected.insert(1, (999, 999));
    assert_eq!(
        map.iter().map(|(&k, &v)| (k, v)).collect::<Vec<_>>(),
        expected
    );
    for (index, (key, value)) in expected.iter().enumerate() {
        assert_eq!(map.get_index_of(key), Some(index));
        assert_eq!(map.get(key), Some(value));
    }
}
