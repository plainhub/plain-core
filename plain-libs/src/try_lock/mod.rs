use core::cell::UnsafeCell;
use core::fmt;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};
#[derive(Default)]
pub struct TryLock<T> {
    is_locked: AtomicBool,
    value: UnsafeCell<T>,
}
impl<T> TryLock<T> {
    #[inline]
    pub const fn new(val: T) -> TryLock<T> {
        TryLock {
            is_locked: AtomicBool::new(false),
            value: UnsafeCell::new(val),
        }
    }
    #[inline]
    pub fn try_lock(&self) -> Option<Locked<T>> {
        unsafe { self.try_lock_explicit_unchecked(Ordering::Acquire, Ordering::Release) }
    }
    #[inline]
    pub fn try_lock_explicit(
        &self,
        lock_order: Ordering,
        unlock_order: Ordering,
    ) -> Option<Locked<T>> {
        match lock_order {
            Ordering::Acquire | Ordering::AcqRel | Ordering::SeqCst => {}
            _ => panic!("lock ordering must be `Acquire`, `AcqRel`, or `SeqCst`"),
        }
        match unlock_order {
            Ordering::Release | Ordering::SeqCst => {}
            _ => panic!("unlock ordering must be `Release` or `SeqCst`"),
        }
        unsafe { self.try_lock_explicit_unchecked(lock_order, unlock_order) }
    }
    #[inline]
    pub unsafe fn try_lock_explicit_unchecked(
        &self,
        lock_order: Ordering,
        unlock_order: Ordering,
    ) -> Option<Locked<T>> {
        if !self.is_locked.swap(true, lock_order) {
            Some(Locked {
                lock: self,
                order: unlock_order,
                _p: PhantomData,
            })
        } else {
            None
        }
    }
    #[inline]
    pub fn into_inner(self) -> T {
        debug_assert!(
            !self.is_locked.load(Ordering::Relaxed),
            "TryLock was mem::forgotten"
        );
        self.value.into_inner()
    }
}
unsafe impl<T: Send> Send for TryLock<T> {}
unsafe impl<T: Send> Sync for TryLock<T> {}
impl<T: fmt::Debug> fmt::Debug for TryLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        struct LockedPlaceholder;
        impl fmt::Debug for LockedPlaceholder {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("<locked>")
            }
        }
        let mut builder = f.debug_struct("TryLock");
        if let Some(locked) = self.try_lock() {
            builder.field("value", &*locked);
        } else {
            builder.field("value", &LockedPlaceholder);
        }
        builder.finish()
    }
}
#[must_use = "TryLock will immediately unlock if not used"]
pub struct Locked<'a, T: 'a> {
    lock: &'a TryLock<T>,
    order: Ordering,
    _p: PhantomData<*mut T>,
}
impl<'a, T> Deref for Locked<'a, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        unsafe { &*self.lock.value.get() }
    }
}
impl<'a, T> DerefMut for Locked<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.value.get() }
    }
}
impl<'a, T> Drop for Locked<'a, T> {
    #[inline]
    fn drop(&mut self) {
        self.lock.is_locked.store(false, self.order);
    }
}
impl<'a, T: fmt::Debug> fmt::Debug for Locked<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/try_lock.rs"]
mod tests;
