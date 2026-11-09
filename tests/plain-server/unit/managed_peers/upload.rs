use super::*;

#[tokio::test(start_paused = true)]
async fn progress_renews_the_deadline_beyond_thirty_seconds_then_receipt_wait_is_bounded() {
    let (progress, updates) = watch::channel(Instant::now());
    let mut stalled = Box::pin(wait_for_stall(updates));
    assert!(futures_util::poll!(stalled.as_mut()).is_pending());
    for _ in 0..4 {
        tokio::time::advance(Duration::from_secs(20)).await;
        progress.send_replace(Instant::now());
        assert!(futures_util::poll!(stalled.as_mut()).is_pending());
    }
    tokio::time::advance(Duration::from_secs(29)).await;
    assert!(futures_util::poll!(stalled.as_mut()).is_pending());
    tokio::time::advance(Duration::from_secs(1)).await;
    assert!(futures_util::poll!(stalled.as_mut()).is_ready());
}

#[tokio::test(start_paused = true)]
async fn progress_before_poll_and_repeated_identical_ticks_do_not_lose_wakes_or_spin() {
    let (progress, updates) = watch::channel(Instant::now());
    let mut stalled = Box::pin(wait_for_stall(updates));
    tokio::time::advance(Duration::from_secs(25)).await;
    for _ in 0..3 {
        progress.send_replace(Instant::now());
    }
    assert!(futures_util::poll!(stalled.as_mut()).is_pending());
    assert!(futures_util::poll!(stalled.as_mut()).is_pending());
    tokio::time::advance(Duration::from_secs(29)).await;
    assert!(futures_util::poll!(stalled.as_mut()).is_pending());
    tokio::time::advance(Duration::from_secs(1)).await;
    assert!(futures_util::poll!(stalled.as_mut()).is_ready());
}

#[tokio::test(start_paused = true)]
async fn unpolled_progress_replaces_an_expired_old_deadline_before_deciding_to_timeout() {
    let (progress, updates) = watch::channel(Instant::now());
    let mut stalled = Box::pin(wait_for_stall(updates));
    assert!(futures_util::poll!(stalled.as_mut()).is_pending());
    tokio::time::advance(Duration::from_secs(20)).await;
    progress.send_replace(Instant::now());
    tokio::time::advance(Duration::from_secs(20)).await;
    assert!(futures_util::poll!(stalled.as_mut()).is_pending());
    tokio::time::advance(Duration::from_secs(9)).await;
    assert!(futures_util::poll!(stalled.as_mut()).is_pending());
    tokio::time::advance(Duration::from_secs(1)).await;
    assert!(futures_util::poll!(stalled.as_mut()).is_ready());
}
