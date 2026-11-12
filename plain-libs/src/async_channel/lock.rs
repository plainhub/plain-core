use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering::SeqCst;
#[derive(Debug)]
pub(crate) struct Lock<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}
pub(crate) struct TryLock<'a, T> {
    __ptr: &'a Lock<T>,
}
// Mutable access requires the successful SeqCst lock acquisition and its guard.
unsafe impl<T: Send> Send for Lock<T> {}
unsafe impl<T: Send> Sync for Lock<T> {}
impl<T> Lock<T> {
    pub(crate) fn new(t: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(t),
        }
    }
    pub(crate) fn try_lock(&self) -> Option<TryLock<'_, T>> {
        if !self.locked.swap(true, SeqCst) {
            Some(TryLock { __ptr: self })
        } else {
            None
        }
    }
}
impl<T> Deref for TryLock<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.__ptr.data.get() }
    }
}
impl<T> DerefMut for TryLock<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.__ptr.data.get() }
    }
}
impl<T> Drop for TryLock<'_, T> {
    fn drop(&mut self) {
        self.__ptr.locked.store(false, SeqCst);
    }
}
#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/async_channel/lock.rs"]
mod tests;
