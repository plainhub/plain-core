pub(super) use self::PopResult::*;
use std::boxed::Box;
use std::cell::UnsafeCell;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::thread;
pub(super) enum PopResult<T> {
    Data(T),
    Empty,
    Inconsistent,
}
struct Node<T> {
    next: AtomicPtr<Self>,
    value: Option<T>,
}
pub(super) struct Queue<T> {
    head: AtomicPtr<Node<T>>,
    tail: UnsafeCell<*mut Node<T>>,
}
// Producers publish nodes with Release; exactly one consumer owns the tail.
unsafe impl<T: Send> Send for Queue<T> {}
unsafe impl<T: Send> Sync for Queue<T> {}
impl<T> Node<T> {
    fn new(v: Option<T>) -> *mut Self {
        Box::into_raw(Box::new(Self {
            next: AtomicPtr::new(ptr::null_mut()),
            value: v,
        }))
    }
}
impl<T> Queue<T> {
    pub(super) fn new() -> Self {
        let stub = Node::new(None);
        Self {
            head: AtomicPtr::new(stub),
            tail: UnsafeCell::new(stub),
        }
    }
    pub(super) fn push(&self, t: T) {
        unsafe {
            let n = Node::new(Some(t));
            let prev = self.head.swap(n, Ordering::AcqRel);
            (*prev).next.store(n, Ordering::Release);
        }
    }
    // The caller must serialize all pops; Acquire observes fully initialized nodes.
    pub(super) unsafe fn pop(&self) -> PopResult<T> {
        unsafe {
            let tail = *self.tail.get();
            let next = (*tail).next.load(Ordering::Acquire);
            if !next.is_null() {
                *self.tail.get() = next;
                assert!((*tail).value.is_none());
                assert!((*next).value.is_some());
                let ret = (*next).value.take().unwrap();
                drop(Box::from_raw(tail));
                return Data(ret);
            }
            if self.head.load(Ordering::Acquire) == tail {
                Empty
            } else {
                Inconsistent
            }
        }
    }
    #[inline]
    pub(super) unsafe fn pop_spin(&self) -> Option<T> {
        loop {
            match unsafe { self.pop() } {
                Empty => return None,
                Data(t) => return Some(t),
                Inconsistent => {
                    thread::yield_now();
                }
            }
        }
    }
}
impl<T> Drop for Queue<T> {
    fn drop(&mut self) {
        unsafe {
            let mut cur = *self.tail.get();
            while !cur.is_null() {
                let next = (*cur).next.load(Ordering::Relaxed);
                drop(Box::from_raw(cur));
                cur = next;
            }
        }
    }
}
