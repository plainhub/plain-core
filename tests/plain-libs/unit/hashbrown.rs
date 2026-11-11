use super::*;
#[test]
fn collision_growth_lookup_and_removal_preserve_every_entry() {
    let mut table = HashTable::new();
    for value in 0..1024 {
        table.insert_unique(0, value, |_| 0);
    }
    for value in 0..1024 {
        assert_eq!(table.find(0, |x| *x == value), Some(&value));
    }
    for value in (0..1024).step_by(2) {
        let entry = table.find_entry(0, |x| *x == value).unwrap();
        let _ = entry.remove();
    }
    assert_eq!(table.len(), 512);
    for value in 0..1024 {
        assert_eq!(
            table.find(0, |x| *x == value).copied(),
            (value % 2 == 1).then_some(value)
        );
    }
}
