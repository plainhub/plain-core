use super::*;

#[test]
fn every_embedded_map_is_readable_and_the_cache_stays_bounded() {
    let data = Data::default();
    assert_eq!(INDEX.0.entries.len(), 2033);
    for key in INDEX.0.entries.keys() {
        let map = data.get(key).unwrap();
        if key.starts_with("countries/") {
            assert!(matches!(&*map, Map::Countries(_)));
        } else {
            assert!(matches!(&*map, Map::Prefixes { .. }));
        }
        let cache = data.cache.lock().unwrap();
        assert!(cache.bytes <= CACHE_BYTES);
        assert!(cache.entries.len() <= CACHE_ENTRIES);
        assert_eq!(
            cache.bytes,
            cache
                .entries
                .iter()
                .map(|(_, map)| map.bytes())
                .sum::<usize>()
        );
    }
}

#[test]
fn duplicate_language_maps_share_the_same_decoded_allocation() {
    let data = Data::default();
    let mut seen = BTreeMap::new();
    let (first, second) = INDEX
        .0
        .entries
        .iter()
        .find_map(|(key, range)| seen.insert(*range, key).map(|first| (first, key)))
        .unwrap();
    assert!(Arc::ptr_eq(
        &data.get(first).unwrap(),
        &data.get(second).unwrap()
    ));
    assert_eq!(data.cache.lock().unwrap().entries.len(), 1);
}

#[test]
fn longest_prefix_wins_even_when_its_description_is_empty() {
    let map = Map::Prefixes {
        entries: vec![(1, 0), (1201, 1), (1201555, 2)],
        descriptions: vec!["country".into(), "area".into(), String::new()],
        lengths: vec![1, 4, 7],
    };
    assert_eq!(map.prefix(1201999), "area");
    assert_eq!(map.prefix(1_201_555 * 10_000 + 100), "");
    assert_eq!(map.prefix(1999), "country");
    assert_eq!(map.prefix(44), "");
    assert_eq!(map.prefix(0), "");
}

#[test]
fn incomplete_invalid_and_trailing_map_bytes_are_rejected() {
    for bytes in [
        b"".as_slice(),
        b"PFX1",
        b"OTHER123",
        b"CTY1\0\0\0\x01",
        b"CTY1\0\0\0\0trailing",
    ] {
        assert!(Map::decode(bytes).is_err());
    }
    let countries = b"CTY1\0\0\0\x01\0\0\0\x02US\0\0\0\x06region";
    assert_eq!(Map::decode(countries).unwrap().country("US"), "region");
    for length in 0..countries.len() {
        assert!(Map::decode(&countries[..length]).is_err());
    }
}
