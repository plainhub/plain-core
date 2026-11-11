use super::*;
#[test]
fn demand_and_cancellation_are_observable_to_the_giver() {
    let (giver, mut taker) = new();
    assert!(!giver.is_wanting());
    taker.want();
    assert!(giver.is_wanting());
    assert!(giver.give());
    assert!(!giver.is_wanting());
    taker.cancel();
    assert!(giver.is_canceled());
    assert!(!giver.give());
}
#[test]
fn dropping_the_taker_closes_a_pending_giver() {
    let (mut giver, taker) = new();
    struct WakeCount(std::sync::atomic::AtomicUsize);
    impl std::task::Wake for WakeCount {
        fn wake(self: std::sync::Arc<Self>) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let count = std::sync::Arc::new(WakeCount(std::sync::atomic::AtomicUsize::new(0)));
    let waker = std::task::Waker::from(count.clone());
    let mut context = std::task::Context::from_waker(&waker);
    assert!(giver.poll_want(&mut context).is_pending());
    drop(taker);
    assert_eq!(count.0.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(matches!(
        giver.poll_want(&mut context),
        std::task::Poll::Ready(Err(_))
    ));
}
