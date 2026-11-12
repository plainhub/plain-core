use super::*;

#[test]
fn shared_fnv_preserves_collision_defense_and_failed_reservation_keeps_entries() {
    let mut map = HeaderMap::<usize>::with_capacity(1024);
    let mask = map.mask;
    let count = DISPLACEMENT_THRESHOLD + 1;
    let mut adjacent = vec![None; count];
    let mut extra = None;
    let mut found = 0;
    for index in 0..1_000_000 {
        let name = HeaderName::from_bytes(format!("x-collision-{index}").as_bytes()).unwrap();
        let position = desired_pos(mask, hash_elem_using(&Danger::Green, &name));
        if position < count {
            if adjacent[position].is_none() {
                adjacent[position] = Some(name);
                found += 1;
            } else if position == 0 && extra.is_none() {
                extra = Some(name);
            }
        }
        if found == count && extra.is_some() {
            break;
        }
    }
    assert_eq!(found, count);
    let mut names: Vec<_> = adjacent.into_iter().map(Option::unwrap).collect();
    names.push(extra.expect("a second initial-bucket key"));
    for (index, name) in names.iter().enumerate() {
        assert_eq!(map.insert(name.clone(), index), None);
    }
    assert!(
        map.danger.is_yellow(),
        "the insertion must displace the whole adjacent cluster"
    );
    let sentinel = HeaderName::from_static("x-trigger-rehash");
    assert_eq!(map.insert(sentinel.clone(), names.len()), None);
    names.push(sentinel);
    assert!(
        map.danger.is_red(),
        "adversarial displacement must select randomized hashing"
    );
    assert!(map.try_reserve(usize::MAX).is_err());
    assert_eq!(map.len(), names.len());
    for (index, name) in names.iter().enumerate() {
        assert_eq!(map.get(name), Some(&index));
    }
}
