use super::*;

#[test]
fn published_vectors_and_incremental_writes_match() {
    for (input, expected) in [
        (&b""[..], 0xcbf29ce484222325),
        (&b"a"[..], 0xaf63dc4c8601ec8c),
        (&b"foobar"[..], 0x85944171f73967e8),
    ] {
        for split in 0..=input.len() {
            let mut hasher = FnvHasher::default();
            hasher.write(&input[..split]);
            hasher.write(&[]);
            hasher.write(&input[split..]);
            assert_eq!(hasher.finish(), expected);
        }
    }
}

#[test]
fn map_aliases_keep_borrowed_lookup_and_owned_key_storage() {
    let mut map = FnvHashMap::default();
    map.insert("entry".to_owned(), 3);
    let pointer = map.get_key_value("entry").unwrap().0.as_ptr();
    assert_eq!(map.insert("entry".to_owned(), 7), Some(3));
    assert_eq!(map.get("entry"), Some(&7));
    assert_eq!(map.get_key_value("entry").unwrap().0.as_ptr(), pointer);
    let mut set = FnvHashSet::default();
    assert!(set.insert("entry".to_owned()));
    assert!(!set.insert("entry".to_owned()));
    assert!(set.remove("entry"));
}
