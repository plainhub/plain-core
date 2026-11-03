use super::*;

fn key(n: u32) -> PathBuf {
    PathBuf::from(format!("/cache/thumbs/ab/{n:04}.jpg"))
}

fn data(n: usize) -> Arc<Vec<u8>> {
    Arc::new(vec![7u8; n])
}

#[test]
fn put_get_and_byte_cap_eviction() {
    let lru = ThumbLru::new(256, 100); // 256 bytes
    lru.put(key(1), data(100));
    lru.put(key(2), data(100));
    lru.put(key(3), data(100)); // 300 > 256 → oldest (1) evicted
    assert!(lru.get(&key(1)).is_none());
    assert!(lru.get(&key(2)).is_some());
    assert!(lru.get(&key(3)).is_some());
    let (n, bytes) = lru.stats();
    assert_eq!(n, 2);
    assert_eq!(bytes, 200);
}

#[test]
fn touch_changes_eviction_order() {
    let lru = ThumbLru::new(256, 100);
    lru.put(key(1), data(100));
    lru.put(key(2), data(100));
    // Touch 1 so 2 becomes the LRU victim.
    let _ = lru.get(&key(1));
    lru.put(key(3), data(100));
    assert!(
        lru.get(&key(2)).is_none(),
        "untouched entry must be evicted"
    );
    assert!(lru.get(&key(1)).is_some());
}

#[test]
fn entry_count_cap() {
    let lru = ThumbLru::new(1 << 20, 3);
    for i in 0..5 {
        lru.put(key(i), data(10));
    }
    let (n, _) = lru.stats();
    assert!(n <= 3, "entries {n} exceed cap");
    assert!(lru.get(&key(0)).is_none(), "oldest evicted");
    assert!(lru.get(&key(4)).is_some(), "newest kept");
}

#[test]
fn replace_same_key_does_not_double_count() {
    let lru = ThumbLru::new(1 << 20, 100);
    lru.put(key(1), data(100));
    lru.put(key(1), data(200));
    let (n, bytes) = lru.stats();
    assert_eq!((n, bytes), (1, 200));
}

#[test]
fn a_full_cache_evicts_without_rescanning_resident_images() {
    let lru = ThumbLru::new(1 << 20, 4096);
    for i in 0..4396 {
        lru.put(key(i), data(64));
    }
    let inner = lru.inner.lock().unwrap();
    assert_eq!(inner.eviction_probes.get(), 300);
    assert_eq!(inner.bytes, 4096 * 64);
}

#[test]
fn replacement_clear_and_slot_reuse_match_a_recency_model() {
    let lru = ThumbLru::new(160, 8);
    let mut model = std::collections::VecDeque::<(PathBuf, Arc<Vec<u8>>)>::new();
    let mut random = 7u64;
    for i in 0..5000 {
        random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
        let path = key((random >> 32) as u32 % 32);
        if i % 97 == 0 {
            lru.clear();
            model.clear();
        } else if random & 1 == 0 {
            let bytes = data((random >> 40) as usize % 240 + 1);
            if let Some(index) = model.iter().position(|(key, _)| key == &path) {
                model.remove(index);
            }
            model.push_back((path.clone(), bytes.clone()));
            while model.len() > 1
                && (model.len() > 8
                    || model.iter().map(|(_, bytes)| bytes.len()).sum::<usize>() > 160)
            {
                model.pop_front();
            }
            lru.put(path, bytes);
        } else {
            let expected = model.iter().position(|(key, _)| key == &path).map(|index| {
                let entry = model.remove(index).unwrap();
                let bytes = entry.1.clone();
                model.push_back(entry);
                bytes
            });
            assert_eq!(lru.get(&path), expected);
        }
        assert_eq!(
            lru.stats(),
            (
                model.len(),
                model.iter().map(|(_, bytes)| bytes.len()).sum()
            )
        );
        let inner = lru.inner.lock().unwrap();
        assert!(inner.entries.len() <= 9);
        let order = std::iter::successors(inner.oldest, |index| {
            inner.entries[*index].as_ref().unwrap().next
        })
        .take(inner.entries.len() + 1)
        .map(|index| inner.entries[index].as_ref().unwrap().key.clone())
        .collect::<Vec<_>>();
        assert_eq!(
            order,
            model.iter().map(|(key, _)| key.clone()).collect::<Vec<_>>()
        );
    }
}

#[test]
fn parallel_access_preserves_capacity_and_retained_bytes() {
    let lru = Arc::new(ThumbLru::new(1024, 16));
    lru.put(key(0), data(64));
    let retained = lru.get(&key(0)).unwrap();
    std::thread::scope(|scope| {
        for worker in 0..8 {
            let lru = lru.clone();
            scope.spawn(move || {
                for i in 0..300 {
                    let path = key(1 + worker * 300 + i);
                    lru.put(path.clone(), data(64));
                    if let Some(bytes) = lru.get(&path) {
                        assert_eq!(bytes.as_slice(), &[7; 64]);
                    }
                }
            });
        }
    });
    assert_eq!(lru.stats(), (16, 1024));
    assert_eq!(retained.as_slice(), &[7; 64]);
    assert!(lru.get(&key(0)).is_none());
}
