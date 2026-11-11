pub struct Pool<T, F = fn() -> T>(std::boxed::Box<inner::Pool<T, F>>);
impl<T, F> Pool<T, F> {
    pub fn new(create: F) -> Pool<T, F> {
        Pool(std::boxed::Box::new(inner::Pool::new(create)))
    }
    pub fn with_capacity(capacity: usize, create: F) -> Pool<T, F> {
        Pool(std::boxed::Box::new(inner::Pool::with_capacity(
            capacity, create,
        )))
    }
}
impl<T: Send, F: Fn() -> T> Pool<T, F> {
    #[inline]
    pub fn get(&self) -> PoolGuard<'_, T, F> {
        PoolGuard(self.0.get())
    }
}
impl<T: core::fmt::Debug, F> core::fmt::Debug for Pool<T, F> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_tuple("Pool").field(&self.0).finish()
    }
}
pub struct PoolGuard<'a, T: Send, F: Fn() -> T>(inner::PoolGuard<'a, T, F>);
impl<'a, T: Send, F: Fn() -> T> PoolGuard<'a, T, F> {
    #[inline]
    pub fn put(this: PoolGuard<'_, T, F>) {
        inner::PoolGuard::put(this.0);
    }
}
impl<'a, T: Send, F: Fn() -> T> core::ops::Deref for PoolGuard<'a, T, F> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        self.0.value()
    }
}
impl<'a, T: Send, F: Fn() -> T> core::ops::DerefMut for PoolGuard<'a, T, F> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        self.0.value_mut()
    }
}
impl<'a, T: Send + core::fmt::Debug, F: Fn() -> T> core::fmt::Debug for PoolGuard<'a, T, F> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_tuple("PoolGuard").field(&self.0).finish()
    }
}
mod inner {
    use core::{
        cell::UnsafeCell,
        panic::{RefUnwindSafe, UnwindSafe},
        sync::atomic::{AtomicUsize, Ordering},
    };
    use std::{boxed::Box, vec, vec::Vec};
    use std::{sync::Mutex, thread_local};
    static COUNTER: AtomicUsize = AtomicUsize::new(3);
    static THREAD_ID_UNOWNED: usize = 0;
    static THREAD_ID_INUSE: usize = 1;
    static THREAD_ID_DROPPED: usize = 2;
    const MAX_POOL_STACKS: usize = 8;
    thread_local!(
        static THREAD_ID: usize = {
            let next = COUNTER.fetch_add(1, Ordering::Relaxed);
            if next == 0 {
                panic!("regex: thread ID allocation space exhausted");
            }
            next
        };
    );
    #[derive(Debug)]
    #[repr(C, align(64))]
    struct CacheLine<T>(T);
    pub(super) struct Pool<T, F> {
        create: F,
        stacks: Vec<CacheLine<Mutex<Vec<Box<T>>>>>,
        owner: AtomicUsize,
        owner_val: UnsafeCell<Option<T>>,
    }
    unsafe impl<T: Send, F: Send + Sync> Sync for Pool<T, F> {}
    impl<T: UnwindSafe, F: UnwindSafe + RefUnwindSafe> UnwindSafe for Pool<T, F> {}
    impl<T: UnwindSafe, F: UnwindSafe + RefUnwindSafe> RefUnwindSafe for Pool<T, F> {}
    impl<T, F> Pool<T, F> {
        pub(super) fn new(create: F) -> Pool<T, F> {
            Pool::with_capacity(MAX_POOL_STACKS, create)
        }
        pub(super) fn with_capacity(capacity: usize, create: F) -> Pool<T, F> {
            let mut stacks = Vec::with_capacity(capacity.max(1));
            for _ in 0..stacks.capacity() {
                stacks.push(CacheLine(Mutex::new(vec![])));
            }
            let owner = AtomicUsize::new(THREAD_ID_UNOWNED);
            let owner_val = UnsafeCell::new(None);
            Pool {
                create,
                stacks,
                owner,
                owner_val,
            }
        }
    }
    impl<T: Send, F: Fn() -> T> Pool<T, F> {
        #[inline]
        pub(super) fn get(&self) -> PoolGuard<'_, T, F> {
            let caller = THREAD_ID.with(|id| *id);
            let owner = self.owner.load(Ordering::Acquire);
            if caller == owner {
                self.owner.store(THREAD_ID_INUSE, Ordering::Release);
                return self.guard_owned(caller);
            }
            self.get_slow(caller, owner)
        }
        #[cold]
        fn get_slow(&self, caller: usize, owner: usize) -> PoolGuard<'_, T, F> {
            if owner == THREAD_ID_UNOWNED {
                let res = self.owner.compare_exchange(
                    THREAD_ID_UNOWNED,
                    THREAD_ID_INUSE,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                );
                if res.is_ok() {
                    unsafe {
                        *self.owner_val.get() = Some((self.create)());
                    }
                    return self.guard_owned(caller);
                }
            }
            let stack_id = caller % self.stacks.len();
            for _ in 0..1 {
                let mut stack = match self.stacks[stack_id].0.try_lock() {
                    Err(_) => continue,
                    Ok(stack) => stack,
                };
                if let Some(value) = stack.pop() {
                    return self.guard_stack(value);
                }
                drop(stack);
                let value = Box::new((self.create)());
                return self.guard_stack(value);
            }
            self.guard_stack_transient(Box::new((self.create)()))
        }
        #[inline]
        fn put_value(&self, value: Box<T>) {
            let caller = THREAD_ID.with(|id| *id);
            let stack_id = caller % self.stacks.len();
            for _ in 0..10 {
                let mut stack = match self.stacks[stack_id].0.try_lock() {
                    Err(_) => continue,
                    Ok(stack) => stack,
                };
                stack.push(value);
                return;
            }
        }
        #[inline]
        fn guard_owned(&self, caller: usize) -> PoolGuard<'_, T, F> {
            PoolGuard {
                pool: self,
                value: Err(caller),
                discard: false,
            }
        }
        #[inline]
        fn guard_stack(&self, value: Box<T>) -> PoolGuard<'_, T, F> {
            PoolGuard {
                pool: self,
                value: Ok(value),
                discard: false,
            }
        }
        #[inline]
        fn guard_stack_transient(&self, value: Box<T>) -> PoolGuard<'_, T, F> {
            PoolGuard {
                pool: self,
                value: Ok(value),
                discard: true,
            }
        }
    }
    impl<T: core::fmt::Debug, F> core::fmt::Debug for Pool<T, F> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_struct("Pool")
                .field("stacks", &self.stacks)
                .field("owner", &self.owner)
                .field("owner_val", &self.owner_val)
                .finish()
        }
    }
    pub(super) struct PoolGuard<'a, T: Send, F: Fn() -> T> {
        pool: &'a Pool<T, F>,
        value: Result<Box<T>, usize>,
        discard: bool,
    }
    impl<'a, T: Send, F: Fn() -> T> PoolGuard<'a, T, F> {
        #[inline]
        pub(super) fn value(&self) -> &T {
            match self.value {
                Ok(ref v) => v,
                Err(id) => unsafe {
                    debug_assert_ne!(THREAD_ID_DROPPED, id);
                    (*self.pool.owner_val.get()).as_ref().unwrap_unchecked()
                },
            }
        }
        #[inline]
        pub(super) fn value_mut(&mut self) -> &mut T {
            match self.value {
                Ok(ref mut v) => v,
                Err(id) => unsafe {
                    debug_assert_ne!(THREAD_ID_DROPPED, id);
                    (*self.pool.owner_val.get()).as_mut().unwrap_unchecked()
                },
            }
        }
        #[inline]
        pub(super) fn put(this: PoolGuard<'_, T, F>) {
            let mut this = core::mem::ManuallyDrop::new(this);
            this.put_imp();
        }
        #[inline(always)]
        fn put_imp(&mut self) {
            match core::mem::replace(&mut self.value, Err(THREAD_ID_DROPPED)) {
                Ok(value) => {
                    if self.discard {
                        return;
                    }
                    self.pool.put_value(value);
                }
                Err(owner) => {
                    assert_ne!(THREAD_ID_DROPPED, owner);
                    self.pool.owner.store(owner, Ordering::Release);
                }
            }
        }
    }
    impl<'a, T: Send, F: Fn() -> T> Drop for PoolGuard<'a, T, F> {
        #[inline]
        fn drop(&mut self) {
            self.put_imp();
        }
    }
    impl<'a, T: Send + core::fmt::Debug, F: Fn() -> T> core::fmt::Debug for PoolGuard<'a, T, F> {
        fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
            f.debug_struct("PoolGuard")
                .field("pool", &self.pool)
                .field("value", &self.value)
                .finish()
        }
    }
}
