use super::*;
#[test]
fn contention_drop_and_mutation_preserve_exclusion() {
    let lock = TryLock::new(1);
    let mut guard = lock.try_lock().unwrap();
    assert!(lock.try_lock().is_none());
    *guard = 7;
    drop(guard);
    assert_eq!(*lock.try_lock().unwrap(), 7);
}
