use super::*;
#[test]
fn statement_cache_refreshes_recency_before_eviction() {
    let mut cache = LruCache::new(2);
    cache.insert("first", 1);
    cache.insert("second", 2);
    assert_eq!(cache.get("first"), Some(&1));
    cache.insert("third", 3);
    assert_eq!(cache.get("second"), None);
    assert_eq!(cache.get("first"), Some(&1));
    assert_eq!(cache.len(), 2);
}
