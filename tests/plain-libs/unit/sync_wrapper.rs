use super::*;
#[test]
fn exclusive_access_reuses_the_original_storage() {
    let mut wrapped = SyncWrapper::new(std::cell::Cell::new(3));
    let pointer = wrapped.get_mut() as *mut _;
    wrapped.get_mut().set(8);
    assert_eq!(wrapped.get_mut() as *mut _, pointer);
    assert_eq!(wrapped.into_inner().get(), 8);
    fn is_sync<T: Sync>() {}
    is_sync::<SyncWrapper<std::cell::Cell<u8>>>();
}
