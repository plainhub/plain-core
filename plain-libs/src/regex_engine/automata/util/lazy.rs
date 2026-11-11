use core::fmt;
pub struct Lazy<T, F = fn() -> T>(lazy::Lazy<T, F>);
impl<T, F> Lazy<T, F> {
    pub const fn new(create: F) -> Lazy<T, F> {
        Lazy(lazy::Lazy::new(create))
    }
}
impl<T, F: Fn() -> T> Lazy<T, F> {
    pub fn get(this: &Lazy<T, F>) -> &T {
        this.0.get()
    }
}
impl<T, F: Fn() -> T> core::ops::Deref for Lazy<T, F> {
    type Target = T;
    fn deref(&self) -> &T {
        Lazy::get(self)
    }
}
impl<T: fmt::Debug, F: Fn() -> T> fmt::Debug for Lazy<T, F> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}
mod lazy {
    use core::{
        fmt,
        marker::PhantomData,
        sync::atomic::{AtomicPtr, Ordering},
    };
    use std::boxed::Box;
    pub(super) struct Lazy<T, F> {
        data: AtomicPtr<T>,
        create: F,
        owned: PhantomData<Box<T>>,
    }
    unsafe impl<T: Send + Sync, F: Send + Sync> Sync for Lazy<T, F> {}
    impl<T, F> Lazy<T, F> {
        pub(super) const fn new(create: F) -> Lazy<T, F> {
            Lazy {
                data: AtomicPtr::new(core::ptr::null_mut()),
                create,
                owned: PhantomData,
            }
        }
    }
    impl<T, F: Fn() -> T> Lazy<T, F> {
        pub(super) fn get(&self) -> &T {
            if let Some(data) = self.poll() {
                return data;
            }
            let data = (self.create)();
            let mut ptr = Box::into_raw(Box::new(data));
            let result = self.data.compare_exchange(
                core::ptr::null_mut(),
                ptr,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
            if let Err(old) = result {
                drop(unsafe { Box::from_raw(ptr) });
                ptr = old;
            }
            unsafe { &*ptr }
        }
        fn poll(&self) -> Option<&T> {
            let ptr = self.data.load(Ordering::Acquire);
            if ptr.is_null() {
                return None;
            }
            Some(unsafe { &*ptr })
        }
    }
    impl<T: fmt::Debug, F: Fn() -> T> fmt::Debug for Lazy<T, F> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.debug_struct("Lazy").field("data", &self.poll()).finish()
        }
    }
    impl<T, F> Drop for Lazy<T, F> {
        fn drop(&mut self) {
            let ptr = *self.data.get_mut();
            if !ptr.is_null() {
                drop(unsafe { Box::from_raw(ptr) });
            }
        }
    }
}
