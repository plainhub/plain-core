use super::Lock;
#[test]
fn smoke() {
    let a = Lock::new(1);
    let mut a1 = a.try_lock().unwrap();
    assert!(a.try_lock().is_none());
    assert_eq!(*a1, 1);
    *a1 = 2;
    drop(a1);
    assert_eq!(*a.try_lock().unwrap(), 2);
    assert_eq!(*a.try_lock().unwrap(), 2);
}
