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

thread_local! { static MUT_ITER_CALLS: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) }; }
pub(crate) fn record_mut_next() {
    let (next, fold) = MUT_ITER_CALLS.get();
    MUT_ITER_CALLS.set((next + 1, fold));
}
pub(crate) fn record_mut_fold() {
    let (next, fold) = MUT_ITER_CALLS.get();
    MUT_ITER_CALLS.set((next, fold + 1));
}
pub(crate) fn reset_mut_iteration() {
    MUT_ITER_CALLS.set((0, 0));
}
pub(crate) fn mut_iteration_calls() -> (usize, usize) {
    MUT_ITER_CALLS.get()
}

#[test]
fn grouped_mutation_visits_partial_groups_and_holes_once() {
    for len in [0, 1, 3, 7, 8, 9, 15, 16, 17, 64, 257] {
        let mut table = HashTable::new();
        for value in 0..len {
            table.insert_unique(0, value, |_| 0);
        }
        for value in (0..len).step_by(3) {
            table
                .find_entry(0, |entry| *entry == value)
                .unwrap()
                .remove();
        }
        let count = table.len();
        let mut iter = table.iter_mut();
        for _ in 0..count / 3 {
            *iter.next().unwrap() += 1000;
        }
        iter.for_each(|value| *value += 1000);
        let mut actual: Vec<_> = table.iter().copied().collect();
        actual.sort_unstable();
        assert_eq!(
            actual,
            (0..len)
                .filter(|value| value % 3 != 0)
                .map(|value| value + 1000)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn standard_bucket_references_preserve_alignment_storage_and_zero_sized_entries() {
    #[repr(align(128))]
    struct Aligned(usize);
    let mut table = HashTable::new();
    for value in 0..73 {
        table.insert_unique(0, Aligned(value), |_| 0);
    }
    let pointers: Vec<_> = (0..73)
        .map(|value| {
            let entry = table.find(0, |entry| entry.0 == value).unwrap();
            let pointer = entry as *const Aligned;
            assert_eq!(pointer.addr() % 128, 0);
            pointer
        })
        .collect();
    table.iter_mut().for_each(|entry| entry.0 += 1000);
    for (value, pointer) in pointers.into_iter().enumerate() {
        let entry = table.find_mut(0, |entry| entry.0 == value + 1000).unwrap();
        assert_eq!(entry as *mut Aligned as *const Aligned, pointer);
    }
    let mut empty = HashTable::new();
    for _ in 0..73 {
        empty.insert_unique(0, (), |_| 0);
    }
    let mut visits = 0;
    empty.iter_mut().for_each(|_| visits += 1);
    assert_eq!(visits, 73);
}

#[test]
fn panicking_group_callback_preserves_the_table_and_untouched_entries() {
    let mut table = HashTable::new();
    for value in 0..64 {
        table.insert_unique(0, value, |_| 0);
    }
    let mut visited = Vec::new();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        table.iter_mut().for_each(|value| {
            visited.push(*value);
            *value += 1000;
            if visited.len() == 3 {
                panic!("synthetic callback failure");
            }
        });
    }));
    assert!(result.is_err());
    assert_eq!(table.len(), 64);
    assert_eq!(visited.len(), 3);
    for value in 0..64 {
        let expected = value + if visited.contains(&value) { 1000 } else { 0 };
        assert_eq!(table.find(0, |entry| *entry == expected), Some(&expected));
    }
}
