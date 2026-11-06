use super::*;

#[test]
fn deeper_levels_restart_under_a_new_parent_item() {
    let level = ListLevel {
        lsid: 7,
        kind: ListType::Numbered,
        start: 1,
    };
    let mut counter = ListCounter::default();
    assert_eq!(counter.next(&level, 0), 1);
    assert_eq!(counter.next(&level, 1), 1);
    assert_eq!(counter.next(&level, 1), 2);
    assert_eq!(counter.next(&level, 0), 2);
    assert_eq!(counter.next(&level, 1), 1);
}

#[test]
fn a_list_starting_elsewhere_counts_from_its_start() {
    let level = ListLevel {
        lsid: 1,
        kind: ListType::Numbered,
        start: 5,
    };
    let mut counter = ListCounter::default();
    assert_eq!(counter.next(&level, 0), 5);
    assert_eq!(counter.next(&level, 0), 6);
}
