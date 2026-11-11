use core::{
    fmt::{self, Debug, Formatter},
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
#[repr(transparent)]
pub struct SyncWrapper<T>(T);
impl<T> SyncWrapper<T> {
    pub const fn new(value: T) -> Self {
        Self(value)
    }
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.0
    }
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut T> {
        unsafe { Pin::map_unchecked_mut(self, |this| &mut this.0) }
    }
    pub fn into_inner(self) -> T {
        self.0
    }
}
unsafe impl<T> Sync for SyncWrapper<T> {}
impl<T> Debug for SyncWrapper<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.pad("SyncWrapper")
    }
}
impl<T: Default> Default for SyncWrapper<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}
impl<T> From<T> for SyncWrapper<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}
pub struct SyncFuture<F> {
    inner: SyncWrapper<F>,
}
impl<F: Future> SyncFuture<F> {
    pub fn new(inner: F) -> Self {
        Self {
            inner: SyncWrapper::new(inner),
        }
    }
    pub fn into_inner(self) -> F {
        self.inner.into_inner()
    }
}
impl<F: Future> Future for SyncFuture<F> {
    type Output = F::Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let inner = unsafe { self.map_unchecked_mut(|x| x.inner.get_mut()) };
        inner.poll(cx)
    }
}
impl<T> Debug for SyncFuture<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.pad("SyncFuture")
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/sync_wrapper.rs"]
mod tests;
