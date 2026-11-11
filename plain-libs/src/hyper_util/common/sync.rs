pub(crate) struct SyncWrapper<T>(T);
impl<T> SyncWrapper<T> {
    pub(crate) fn new(value: T) -> Self {
        Self(value)
    }
    pub(crate) fn get_mut(&mut self) -> &mut T {
        &mut self.0
    }
    pub(crate) fn into_inner(self) -> T {
        self.0
    }
}
unsafe impl<T: Send> Sync for SyncWrapper<T> {}
