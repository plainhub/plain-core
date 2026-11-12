use super::*;
#[path = "../support/poll.rs"]
mod support;
use std::{
    sync::Arc,
    task::{RawWaker, RawWakerVTable, Waker},
};

#[test]
fn registration_replacement_take_and_wake_have_exact_ownership() {
    let cell = AtomicWaker::new();
    let (a, wa) = support::counter();
    let (b, wb) = support::counter();
    cell.wake();
    assert_eq!(a.count(), 0);
    cell.register(&wa);
    assert_eq!(Arc::strong_count(&a), 3);
    cell.register(&wa);
    assert_eq!(Arc::strong_count(&a), 3);
    cell.register(&wb);
    assert_eq!(Arc::strong_count(&a), 2);
    let taken = cell.take().unwrap();
    assert_eq!(a.count() + b.count(), 0);
    assert!(cell.take().is_none());
    assert_eq!(cell.state.load(Acquire), WAITING);
    taken.wake();
    assert_eq!(b.count(), 1);
    cell.wake();
    assert_eq!(b.count(), 1);
}

#[test]
fn all_short_operation_sequences_match_a_single_pending_waker_model() {
    for sequence in 0usize..1024 {
        let cell = AtomicWaker::new();
        let (a, wa) = support::counter();
        let (b, wb) = support::counter();
        let mut pending = None;
        let mut expected = [0, 0];
        let mut code = sequence;
        for _ in 0..5 {
            match code & 3 {
                0 => {
                    cell.register(&wa);
                    pending = Some(0);
                }
                1 => {
                    cell.register(&wb);
                    pending = Some(1);
                }
                2 => {
                    cell.wake();
                    if let Some(i) = pending.take() {
                        expected[i] += 1;
                    }
                }
                _ => {
                    drop(cell.take());
                    pending = None;
                }
            }
            code >>= 2;
            assert_eq!([a.count(), b.count()], expected);
            assert_eq!(cell.state.load(Acquire), WAITING);
        }
    }
}

struct Reentrant {
    owner: Arc<AtomicWaker>,
    wakes: std::sync::atomic::AtomicUsize,
}
unsafe fn clone_reentrant(ptr: *const ()) -> RawWaker {
    // The raw pointer owns one Arc count; cloning increments before publishing another raw pointer.
    let owner = unsafe { &*ptr.cast::<Reentrant>() };
    owner.owner.wake();
    unsafe {
        Arc::increment_strong_count(ptr.cast::<Reentrant>());
    }
    RawWaker::new(ptr, &REENTRANT)
}
unsafe fn wake_reentrant(ptr: *const ()) {
    // This vtable operation consumes exactly its owned Arc count.
    let owner = unsafe { Arc::from_raw(ptr.cast::<Reentrant>()) };
    owner
        .wakes
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}
unsafe fn wake_ref_reentrant(ptr: *const ()) {
    let owner = unsafe { &*ptr.cast::<Reentrant>() };
    owner
        .wakes
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}
unsafe fn drop_reentrant(ptr: *const ()) {
    unsafe {
        drop(Arc::from_raw(ptr.cast::<Reentrant>()));
    }
}
static REENTRANT: RawWakerVTable = RawWakerVTable::new(
    clone_reentrant,
    wake_reentrant,
    wake_ref_reentrant,
    drop_reentrant,
);

#[test]
fn a_wake_during_registration_is_delivered_once_and_releases_the_cell() {
    let cell = Arc::new(AtomicWaker::new());
    let owner = Arc::new(Reentrant {
        owner: cell.clone(),
        wakes: std::sync::atomic::AtomicUsize::new(0),
    });
    let raw = RawWaker::new(Arc::into_raw(owner.clone()).cast(), &REENTRANT);
    // The vtable retains and consumes exactly one Arc count for every Waker owner.
    let waker = unsafe { Waker::from_raw(raw) };
    cell.register(&waker);
    assert_eq!(owner.wakes.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(cell.state.load(Acquire), WAITING);
    assert!(cell.take().is_none());
}

#[test]
fn concurrent_wakers_deliver_a_registration_once() {
    for _ in 0..32 {
        let cell = Arc::new(AtomicWaker::new());
        let (count, waker) = support::counter();
        cell.register(&waker);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let cell = &cell;
                scope.spawn(move || cell.wake());
            }
        });
        assert_eq!(count.count(), 1);
        assert_eq!(cell.state.load(Acquire), WAITING);
    }
}
