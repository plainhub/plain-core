use super::*;
#[path = "../../support/poll.rs"]
mod support;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[test]
fn send_is_zero_copy_and_wakes_the_latest_receiver_only() {
    let (tx, mut rx) = channel();
    let (a, wa) = support::counter();
    let (b, wb) = support::counter();
    assert!(
        Pin::new(&mut rx)
            .poll(&mut Context::from_waker(&wa))
            .is_pending()
    );
    assert!(
        Pin::new(&mut rx)
            .poll(&mut Context::from_waker(&wb))
            .is_pending()
    );
    let bytes: Vec<u8> = (0..1024).map(|n| (n % 251) as u8).collect();
    let ptr = bytes.as_ptr();
    tx.send(bytes).unwrap();
    assert_eq!([a.count(), b.count()], [0, 1]);
    let Poll::Ready(Ok(received)) = Pin::new(&mut rx).poll(&mut Context::from_waker(&wb)) else {
        panic!("missing payload");
    };
    assert_eq!(received.as_ptr(), ptr);
    assert_eq!(received.len(), 1024);
    assert!(rx.is_terminated());
}

#[test]
fn canceled_receiver_returns_the_original_payload_and_sender_drop_wakes_receiver() {
    let (tx, rx) = channel::<Vec<u8>>();
    assert!(!tx.is_canceled());
    drop(rx);
    assert!(tx.is_canceled());
    let data = vec![7; 256];
    let ptr = data.as_ptr();
    assert_eq!(tx.send(data).unwrap_err().as_ptr(), ptr);
    let (tx, mut rx) = channel::<u8>();
    let (count, waker) = support::counter();
    let mut cx = Context::from_waker(&waker);
    assert!(Pin::new(&mut rx).poll(&mut cx).is_pending());
    drop(tx);
    assert_eq!(count.count(), 1);
    assert_eq!(Pin::new(&mut rx).poll(&mut cx), Poll::Ready(Err(Canceled)));
    assert!(rx.is_terminated());
}

struct Dropped(Arc<AtomicUsize>);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
#[test]
fn concurrent_send_cancel_and_unconsumed_payload_drop_exactly_once() {
    for _ in 0..256 {
        let drops = Arc::new(AtomicUsize::new(0));
        let (tx, rx) = channel();
        std::thread::scope(|scope| {
            let drops = drops.clone();
            scope.spawn(move || {
                let _ = tx.send(Dropped(drops));
            });
            scope.spawn(move || drop(rx));
        });
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn unused_cancellation_subscription_storage_is_absent() {
    let slot = std::mem::size_of::<Lock<Option<Waker>>>();
    let original =
        (std::mem::size_of::<AtomicBool>() + std::mem::size_of::<Lock<Option<u64>>>() + 2 * slot)
            .next_multiple_of(std::mem::align_of::<Inner<u64>>());
    assert!(std::mem::size_of::<Inner<u64>>() + slot <= original);
}
