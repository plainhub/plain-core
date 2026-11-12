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

thread_local! { static BULK_CHOICES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
pub(crate) fn record_bulk_choice() {
    BULK_CHOICES.set(BULK_CHOICES.get() + 1);
}

#[test]
fn ordered_shifts_select_grouped_traversal_and_skip_oversized_sparse_tables() {
    let mut map = IndexMap::with_capacity(2048);
    for key in 0..2048 {
        map.insert(key, key);
    }
    crate::hashbrown::tests::reset_mut_iteration();
    assert_eq!(map.shift_remove(&0), Some(0));
    assert_eq!(crate::hashbrown::tests::mut_iteration_calls(), (0, 1));
    crate::hashbrown::tests::reset_mut_iteration();
    assert_eq!(map.shift_remove(&128), Some(128));
    assert_eq!(crate::hashbrown::tests::mut_iteration_calls(), (0, 1));
    for (len, key) in [(512, 256), (2048, 1024), (8192, 4096)] {
        let mut balanced = IndexMap::with_capacity(len);
        for key in 0..len {
            balanced.insert(key, key);
        }
        crate::hashbrown::tests::reset_mut_iteration();
        BULK_CHOICES.set(0);
        assert_eq!(balanced.shift_remove(&key), Some(key));
        assert_eq!(crate::hashbrown::tests::mut_iteration_calls(), (0, 0));
        assert_eq!(BULK_CHOICES.get(), 1);
    }
    let mut sparse = IndexMap::with_capacity(16384);
    for key in 0..128 {
        sparse.insert(key, key);
    }
    crate::hashbrown::tests::reset_mut_iteration();
    BULK_CHOICES.set(0);
    assert_eq!(sparse.shift_remove(&0), Some(0));
    assert_eq!(BULK_CHOICES.get(), 1);
    assert_eq!(crate::hashbrown::tests::mut_iteration_calls(), (0, 0));
    for key in 1..128 {
        assert_eq!(sparse.get_index_of(&key), Some((key - 1) as usize));
    }
}

#[derive(Default)]
struct CollisionHasher;
impl std::hash::Hasher for CollisionHasher {
    fn finish(&self) -> u64 {
        0
    }
    fn write(&mut self, _: &[u8]) {}
}

#[test]
fn ordered_mutations_match_an_independent_vector_under_collisions() {
    for reserve in [0, 64, 2048, 16384] {
        let mut map = IndexMap::with_capacity_and_hasher(
            reserve,
            std::hash::BuildHasherDefault::<CollisionHasher>::default(),
        );
        let mut expected: Vec<(u64, u64)> = (0..256).map(|key| (key, key * 3)).collect();
        for &(key, value) in &expected {
            map.insert(key, value);
        }
        for (from, to) in [(0, 255), (255, 0), (1, 254), (254, 1)] {
            let entry = expected.remove(from);
            expected.insert(to, entry);
            map.move_index(from, to);
            assert_eq!(
                map.iter()
                    .map(|(&key, &value)| (key, value))
                    .collect::<Vec<_>>(),
                expected
            );
            for (index, &(key, value)) in expected.iter().enumerate() {
                assert_eq!(map.get_index_of(&key), Some(index));
                assert_eq!(map.get(&key), Some(&value));
            }
        }
        let mut seed = 7_u64;
        for step in 0..1500 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let key = (seed >> 32) % 384;
            match step % 6 {
                0 => {
                    let position = expected.iter().position(|&(entry, _)| entry == key);
                    let prior =
                        position.map(|position| std::mem::replace(&mut expected[position].1, seed));
                    if position.is_none() {
                        expected.push((key, seed));
                    }
                    assert_eq!(map.insert(key, seed), prior);
                }
                1 => {
                    let prior = expected
                        .iter()
                        .position(|&(entry, _)| entry == key)
                        .map(|position| expected.remove(position).1);
                    assert_eq!(map.shift_remove(&key), prior);
                }
                2 => {
                    let index = (seed as usize) % (expected.len() + 2);
                    let prior = if index < expected.len() {
                        Some(expected.remove(index))
                    } else {
                        None
                    };
                    assert_eq!(map.shift_remove_index(index), prior);
                }
                3 if !expected.is_empty() => {
                    let from = seed as usize % expected.len();
                    let to = (seed >> 16) as usize % expected.len();
                    let entry = expected.remove(from);
                    expected.insert(to, entry);
                    map.move_index(from, to);
                }
                4 => {
                    let index = if expected.is_empty() {
                        0
                    } else {
                        seed as usize % expected.len()
                    };
                    let prior = expected
                        .iter()
                        .position(|&(entry, _)| entry == key)
                        .map(|position| expected.remove(position).1);
                    expected.insert(index, (key, seed));
                    assert_eq!(map.shift_insert(index, key, seed), prior);
                }
                _ => {
                    let prior = expected
                        .iter()
                        .position(|&(entry, _)| entry == key)
                        .map(|position| expected.swap_remove(position).1);
                    assert_eq!(map.swap_remove(&key), prior);
                }
            }
            assert_eq!(
                map.iter()
                    .map(|(&key, &value)| (key, value))
                    .collect::<Vec<_>>(),
                expected
            );
            for (index, &(key, value)) in expected.iter().enumerate() {
                assert_eq!(map.get_index_of(&key), Some(index));
                assert_eq!(map.get(&key), Some(&value));
            }
        }
    }
}

#[test]
fn ordered_removal_moves_owned_handles_without_cloning_payloads_or_dropping_twice() {
    struct Payload {
        bytes: Box<[u8; 32]>,
        drops: std::rc::Rc<std::cell::Cell<usize>>,
    }
    impl Drop for Payload {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }
    let drops = std::rc::Rc::new(std::cell::Cell::new(0));
    let mut map = IndexMap::with_capacity(1024);
    for key in 0..1024 {
        map.insert(
            format!("key{key}"),
            Payload {
                bytes: Box::new([key as u8; 32]),
                drops: drops.clone(),
            },
        );
    }
    let (key, payload) = map.get_key_value("key700").unwrap();
    let key_pointer = key.as_ptr();
    let payload_pointer = payload.bytes.as_ptr();
    let capacity = map.capacity();
    for key in 0..128 {
        drop(map.shift_remove(format!("key{key}").as_str()).unwrap());
    }
    assert_eq!(drops.get(), 128);
    assert_eq!(map.capacity(), capacity);
    let (key, payload) = map.get_key_value("key700").unwrap();
    assert_eq!(key.as_ptr(), key_pointer);
    assert_eq!(payload.bytes.as_ptr(), payload_pointer);
    assert_eq!(map.get_index_of("key700"), Some(572));
    drop(map);
    assert_eq!(drops.get(), 1024);
}
